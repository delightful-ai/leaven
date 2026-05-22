use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;
use clap::{Args as ClapArgs, ValueEnum};
use serde::Serialize;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone, Debug, ClapArgs)]
pub struct Args {
    /// Benchmark case, formatted NAME:FILES:BYTES. May be repeated.
    #[arg(long = "case", value_parser = parse_case)]
    cases: Vec<BenchCase>,
    /// Iterations per case.
    #[arg(long, default_value_t = 3)]
    iterations: usize,
    /// Child revisions to create and reconstruct per sample.
    #[arg(long, default_value_t = 0)]
    intermediate_count: usize,
    /// Parallel workers. Defaults to half of logical CPUs.
    #[arg(long)]
    jobs: Option<usize>,
    /// Execution environment for synthetic samples.
    #[arg(long, value_enum, default_value_t = EnvironmentKind::Local)]
    environment: EnvironmentKind,
    /// JSON report path.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Skip cargo trust tests and run only synthetic samples.
    #[arg(long)]
    skip_trust_tests: bool,
    /// Keep generated work directories under target/git-trust-lane/work.
    #[arg(long)]
    keep_workdirs: bool,
    /// Print every git/cargo command.
    #[arg(long)]
    verbose: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum EnvironmentKind {
    Local,
    Firkin,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct BenchCase {
    name: String,
    file_count: usize,
    file_bytes: usize,
}

impl BenchCase {
    const fn total_bytes(&self) -> usize {
        self.file_count * self.file_bytes
    }
}

#[derive(Clone, Debug)]
struct GitTrustTask {
    environment: EnvironmentKind,
    samples: Vec<GitTrustSample>,
    options: RunOptions,
}

#[derive(Clone, Debug)]
struct GitTrustSample {
    case: BenchCase,
    iteration: usize,
    intermediate_count: usize,
}

#[derive(Clone, Debug)]
struct RunOptions {
    jobs: usize,
    report_path: PathBuf,
    work_root: PathBuf,
    keep_workdirs: bool,
    verbose: bool,
}

#[derive(Clone, Debug)]
struct LocalGitSolver {
    repo_root: PathBuf,
}

#[derive(Clone, Copy, Debug)]
struct TrustScorer;

#[derive(Debug, Serialize)]
struct BenchReport {
    generated_at: String,
    task: TaskReport,
    host: HostReport,
    commands: Vec<CommandReport>,
    samples: Vec<SampleReport>,
    summary: BTreeMap<String, SummaryReport>,
    resource_usage: ResourceReport,
}

#[derive(Debug, Serialize)]
struct TaskReport {
    name: &'static str,
    structure: &'static str,
    environment: EnvironmentKind,
    sample_count: usize,
    intermediate_count: usize,
}

#[derive(Debug, Serialize)]
struct HostReport {
    logical_cpus: usize,
    jobs: usize,
    os: String,
    arch: String,
}

#[derive(Debug, Serialize)]
struct CommandReport {
    command: Vec<String>,
    seconds: f64,
}

#[derive(Clone, Debug, Serialize)]
struct SampleReport {
    case: BenchCase,
    iteration: usize,
    setup_seconds: f64,
    projection_seconds: f64,
    materialize_seconds: f64,
    readback_seconds: f64,
    durable_kib: u64,
    workspace_kib: u64,
    imported_child: String,
    intermediate_chain: Option<IntermediateChainReport>,
    score: TrustScore,
}

#[derive(Clone, Debug, Serialize)]
struct IntermediateChainReport {
    count: usize,
    save_total_seconds: f64,
    save_mean_seconds: f64,
    restore_total_seconds: f64,
    restore_mean_seconds: f64,
    restore_max_seconds: f64,
    changed_bytes: usize,
    durable_before_kib: u64,
    durable_after_kib: u64,
    durable_growth_kib: u64,
    storage_amplification: f64,
    revisions: Vec<IntermediateRevisionReport>,
}

#[derive(Clone, Debug, Serialize)]
struct IntermediateRevisionReport {
    index: usize,
    commit: String,
    save_seconds: f64,
    restore_seconds: f64,
    changed_bytes: usize,
    restored_head_matches: bool,
    restored_content_matches: bool,
}

#[derive(Clone, Debug, Serialize)]
struct TrustScore {
    passed: bool,
    projection: ProjectionTrustScore,
    imported_child_present: bool,
}

#[derive(Clone, Debug, Serialize)]
struct ProjectionTrustScore {
    hidden_ref_absent: bool,
    hidden_object_absent: bool,
    alternates_absent: bool,
}

#[derive(Debug, Serialize)]
struct SummaryReport {
    file_count: usize,
    file_bytes: usize,
    total_mib: f64,
    setup_mean_seconds: f64,
    projection_mean_seconds: f64,
    materialize_mean_seconds: f64,
    readback_mean_seconds: f64,
    durable_kib_mean: f64,
    workspace_kib_mean: f64,
}

#[derive(Debug, Serialize)]
struct ResourceReport {
    wall_seconds: f64,
}

struct SamplePaths {
    root: PathBuf,
    source: PathBuf,
    durable: PathBuf,
    workspace: PathBuf,
    projection: PathBuf,
}

struct SolverOutput {
    case: BenchCase,
    iteration: usize,
    setup_seconds: f64,
    projection_seconds: f64,
    materialize_seconds: f64,
    readback_seconds: f64,
    durable_kib: u64,
    workspace_kib: u64,
    imported_child: String,
    intermediate_chain: Option<IntermediateChainReport>,
    hidden_ref_absent: bool,
    hidden_object_absent: bool,
    alternates_absent: bool,
}

pub fn run(args: Args) -> Result<()> {
    if args.environment == EnvironmentKind::Firkin {
        return Err(
            "Firkin benchmark execution is not wired yet; run the existing Firkin proof tests separately, or use --environment local for this benchmark lane"
                .into(),
        );
    }

    let repo_root = repo_root()?;
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let jobs = args.jobs.unwrap_or_else(half_parallelism).max(1);
    let report_path = match args.out {
        Some(path) if path.is_absolute() => path,
        Some(path) => repo_root.join(path),
        None => repo_root
            .join("target/git-trust-lane")
            .join(format!("{stamp}.json")),
    };
    let work_root = report_path
        .parent()
        .ok_or("report path has no parent")?
        .join("work")
        .join(&stamp);
    fs::create_dir_all(&work_root)?;

    let cases = if args.cases.is_empty() {
        default_cases()
    } else {
        args.cases
    };
    if args.iterations == 0 {
        return Err("--iterations must be positive".into());
    }

    let samples = samples(cases, args.iterations, args.intermediate_count);
    let options = RunOptions {
        jobs,
        report_path,
        work_root,
        keep_workdirs: args.keep_workdirs,
        verbose: args.verbose,
    };
    let task = GitTrustTask {
        environment: args.environment,
        samples,
        options,
    };

    println!("repo: {}", repo_root.display());
    println!(
        "jobs: {} of {} logical CPUs",
        task.options.jobs,
        logical_cpus()
    );
    println!("environment: {:?}", task.environment);
    println!("samples: {}", task.samples.len());
    println!("report: {}", task.options.report_path.display());

    let commands = if args.skip_trust_tests {
        Vec::new()
    } else {
        run_trust_tests(&repo_root, task.options.jobs, task.options.verbose)?
    };

    let solver = LocalGitSolver { repo_root };
    println!(
        "synthetic benchmark: {} GitProgram samples",
        task.samples.len()
    );
    let start = Instant::now();
    let reports = run_samples(&task, &solver)?;
    let report = BenchReport {
        generated_at: stamp,
        task: TaskReport {
            name: "git-trust-bench",
            structure: "task/sample/solver/scorer/environment/report",
            environment: task.environment,
            sample_count: task.samples.len(),
            intermediate_count: args.intermediate_count,
        },
        host: HostReport {
            logical_cpus: logical_cpus(),
            jobs: task.options.jobs,
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
        },
        commands,
        summary: summarize(&reports),
        samples: reports,
        resource_usage: ResourceReport {
            wall_seconds: start.elapsed().as_secs_f64(),
        },
    };

    write_report(&task.options.report_path, &report)?;
    print_summary(&report.summary);
    if task.options.keep_workdirs {
        println!("kept workdirs: {}", task.options.work_root.display());
    } else {
        fs::remove_dir_all(&task.options.work_root)?;
    }
    println!("wrote report: {}", task.options.report_path.display());
    Ok(())
}

fn run_trust_tests(repo_root: &Path, jobs: usize, verbose: bool) -> Result<Vec<CommandReport>> {
    let commands = [
        vec![
            "cargo",
            "test",
            "-p",
            "leaven-workspace-git",
            "--test",
            "git_projection",
        ],
        vec![
            "cargo",
            "test",
            "-p",
            "leaven-agentic-git",
            "--test",
            "git_program_materializer",
        ],
        vec![
            "cargo",
            "test",
            "-p",
            "leaven-workspace-firkin",
            "--test",
            "firkin_git_e2e",
        ],
    ];
    let mut reports = Vec::new();
    for command in commands {
        let command = command.into_iter().map(str::to_owned).collect::<Vec<_>>();
        println!("trust test: {}", command.join(" "));
        let started = Instant::now();
        run_command(
            Command::new(&command[0])
                .args(&command[1..])
                .current_dir(repo_root)
                .env("CARGO_BUILD_JOBS", jobs.to_string())
                .env("NEXTEST_TEST_THREADS", jobs.to_string()),
            verbose,
        )?;
        reports.push(CommandReport {
            command,
            seconds: started.elapsed().as_secs_f64(),
        });
    }
    Ok(reports)
}

fn run_samples(task: &GitTrustTask, solver: &LocalGitSolver) -> Result<Vec<SampleReport>> {
    let queue = Arc::new(Mutex::new(VecDeque::from(task.samples.clone())));
    let results = Arc::new(Mutex::new(Vec::<Result<SampleReport>>::new()));
    let worker_count = task.options.jobs.min(task.samples.len().max(1));
    thread::scope(|scope| {
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let results = Arc::clone(&results);
            let work_root = task.options.work_root.clone();
            let solver = solver.clone();
            scope.spawn(move || {
                loop {
                    let sample = {
                        let mut queue = queue.lock().expect("sample queue mutex poisoned");
                        queue.pop_front()
                    };
                    let Some(sample) = sample else {
                        break;
                    };
                    let report = solver.solve(&work_root, sample).and_then(|output| {
                        let report = TrustScorer::score(output)?;
                        println!(
                            "done {}#{}: setup={:.3}s project={:.3}s materialize={:.3}s readback={:.3}s",
                            report.case.name,
                            report.iteration,
                            report.setup_seconds,
                            report.projection_seconds,
                            report.materialize_seconds,
                            report.readback_seconds
                        );
                        Ok(report)
                    });
                    results
                        .lock()
                        .expect("result mutex poisoned")
                        .push(report);
                }
            });
        }
    });

    let mut reports = Vec::new();
    for result in Arc::into_inner(results)
        .expect("workers still hold result mutex")
        .into_inner()
        .expect("result mutex poisoned")
    {
        reports.push(result?);
    }
    reports.sort_by(|left, right| {
        left.case
            .name
            .cmp(&right.case.name)
            .then(left.iteration.cmp(&right.iteration))
    });
    Ok(reports)
}

impl LocalGitSolver {
    fn solve(&self, work_root: &Path, sample: GitTrustSample) -> Result<SolverOutput> {
        let paths = SamplePaths::new(work_root, &sample.case, sample.iteration);
        fs::create_dir_all(&paths.root)?;

        let started = Instant::now();
        create_source_repo(&paths.source, &sample.case)?;
        git(&self.repo_root, ["clone", "--bare", "--no-local"])
            .arg(&paths.source)
            .arg(&paths.durable)
            .status_checked()?;
        let parent = git_output(&paths.source, ["rev-parse", "main"])?;
        let hidden = git_output(&paths.source, ["rev-parse", "refs/heads/hidden/eval"])?;
        let setup_seconds = seconds(started.elapsed());

        let started = Instant::now();
        let score_inputs = create_projection(&paths.source, &paths.projection, hidden.trim())?;
        let projection_seconds = seconds(started.elapsed());

        let started = Instant::now();
        let checkout = materialize_program(&paths.durable, &paths.workspace, parent.trim())?;
        let materialize_seconds = seconds(started.elapsed());

        let started = Instant::now();
        let (imported_child, intermediate_chain) = if sample.intermediate_count == 0 {
            (
                mutate_and_import_child(&paths.durable, &checkout, parent.trim())?,
                None,
            )
        } else {
            run_intermediate_chain(
                &paths.durable,
                &checkout,
                &paths.workspace,
                parent.trim(),
                sample.intermediate_count,
            )?
        };
        let readback_seconds = seconds(started.elapsed());

        Ok(SolverOutput {
            case: sample.case,
            iteration: sample.iteration,
            setup_seconds,
            projection_seconds,
            materialize_seconds,
            readback_seconds,
            durable_kib: du_kib(&paths.durable)?,
            workspace_kib: du_kib(&paths.workspace)?,
            imported_child,
            intermediate_chain,
            hidden_ref_absent: score_inputs.hidden_ref_absent,
            hidden_object_absent: score_inputs.hidden_object_absent,
            alternates_absent: score_inputs.alternates_absent,
        })
    }
}

impl TrustScorer {
    fn score(output: SolverOutput) -> Result<SampleReport> {
        let imported_child_present = !output.imported_child.is_empty();
        let intermediate_chain_passed = output.intermediate_chain.as_ref().is_none_or(|chain| {
            chain
                .revisions
                .iter()
                .all(|revision| revision.restored_head_matches && revision.restored_content_matches)
        });
        let passed = output.hidden_ref_absent
            && output.hidden_object_absent
            && output.alternates_absent
            && imported_child_present
            && intermediate_chain_passed;
        if !passed {
            return Err(format!(
                "sample {}#{} failed trust score",
                output.case.name, output.iteration
            )
            .into());
        }
        Ok(SampleReport {
            case: output.case,
            iteration: output.iteration,
            setup_seconds: output.setup_seconds,
            projection_seconds: output.projection_seconds,
            materialize_seconds: output.materialize_seconds,
            readback_seconds: output.readback_seconds,
            durable_kib: output.durable_kib,
            workspace_kib: output.workspace_kib,
            imported_child: output.imported_child,
            intermediate_chain: output.intermediate_chain,
            score: TrustScore {
                passed,
                projection: ProjectionTrustScore {
                    hidden_ref_absent: output.hidden_ref_absent,
                    hidden_object_absent: output.hidden_object_absent,
                    alternates_absent: output.alternates_absent,
                },
                imported_child_present,
            },
        })
    }
}

impl SamplePaths {
    fn new(work_root: &Path, case: &BenchCase, iteration: usize) -> Self {
        let root = work_root.join(format!("{}-{iteration}", case.name));
        Self {
            source: root.join("source"),
            durable: root.join("program.git"),
            workspace: root.join("workspace"),
            projection: root.join("archive.git"),
            root,
        }
    }
}

struct ProjectionScoreInputs {
    hidden_ref_absent: bool,
    hidden_object_absent: bool,
    alternates_absent: bool,
}

fn create_source_repo(source: &Path, case: &BenchCase) -> Result<()> {
    fs::create_dir_all(source)?;
    git(source, ["init", "--initial-branch=main"]).status_checked()?;
    git(source, ["config", "user.name", "Leaven Benchmark"]).status_checked()?;
    git(source, ["config", "user.email", "leaven@example.invalid"]).status_checked()?;
    let data_dir = source.join("src");
    fs::create_dir_all(&data_dir)?;
    for index in 0..case.file_count {
        write_payload(
            &data_dir.join(format!("file-{index:05}.dat")),
            case.file_bytes,
            &case.name,
            index,
        )?;
    }
    git(source, ["add", "src"]).status_checked()?;
    git(source, ["commit", "-m", "base"]).status_checked()?;
    git(source, ["checkout", "-b", "hidden/eval"]).status_checked()?;
    write_payload(
        &source.join("hidden-evaluator-target.dat"),
        8192,
        &case.name,
        999_999,
    )?;
    git(source, ["add", "hidden-evaluator-target.dat"]).status_checked()?;
    git(source, ["commit", "-m", "hidden evaluator target"]).status_checked()?;
    git(source, ["checkout", "main"]).status_checked()?;
    Ok(())
}

fn create_projection(
    source: &Path,
    projection: &Path,
    hidden_commit: &str,
) -> Result<ProjectionScoreInputs> {
    git_no_cwd(["init", "--bare"])
        .arg(projection)
        .status_checked()?;
    git(projection, ["fetch"])
        .arg(source)
        .arg("+refs/heads/main:refs/heads/program/base")
        .status_checked()?;
    git(projection, ["fsck", "--strict"]).status_checked()?;
    let hidden_ref_absent = !git_success(
        projection,
        ["show-ref", "--verify", "refs/heads/hidden/eval"],
    )?;
    let hidden_object_absent = !git_success(projection, ["cat-file", "-e", hidden_commit])?;
    let alternates_absent = !projection.join("objects/info/alternates").exists();
    if !hidden_ref_absent || !hidden_object_absent || !alternates_absent {
        return Err("projection leaked hidden ref, hidden object, or alternates".into());
    }
    Ok(ProjectionScoreInputs {
        hidden_ref_absent,
        hidden_object_absent,
        alternates_absent,
    })
}

fn materialize_program(durable: &Path, workspace: &Path, parent: &str) -> Result<PathBuf> {
    let checkout = workspace.join("repos/program");
    fs::create_dir_all(checkout.parent().ok_or("checkout has no parent")?)?;
    let bundle = workspace.join("materialization.bundle");
    let temp_ref = format!("refs/leaven/materialize/{parent}");
    git(durable, ["update-ref"])
        .arg(&temp_ref)
        .arg(parent)
        .status_checked()?;
    let create_result = git(durable, ["bundle", "create"])
        .arg(&bundle)
        .arg(&temp_ref)
        .status_checked();
    git(durable, ["update-ref", "-d"])
        .arg(&temp_ref)
        .status_checked()?;
    create_result?;
    git_no_cwd(["init"]).arg(&checkout).status_checked()?;
    let materialized_ref = format!("refs/leaven/materialized/{parent}");
    git(&checkout, ["fetch", "--no-tags", "--no-write-fetch-head"])
        .arg(&bundle)
        .arg(format!("+{parent}:{materialized_ref}"))
        .status_checked()?;
    fs::remove_file(&bundle)?;
    git(&checkout, ["checkout", "--detach", parent]).status_checked()?;
    git(&checkout, ["ls-files", "-z"]).status_checked()?;
    Ok(checkout)
}

fn mutate_and_import_child(durable: &Path, checkout: &Path, parent: &str) -> Result<String> {
    mutate_and_import_child_step(durable, checkout, parent, 1).map(|child| child.commit)
}

struct ImportedStep {
    commit: String,
    save_seconds: f64,
    changed_bytes: usize,
}

fn mutate_and_import_child_step(
    durable: &Path,
    checkout: &Path,
    parent: &str,
    step: usize,
) -> Result<ImportedStep> {
    let started = Instant::now();
    git(checkout, ["config", "user.name", "Leaven Benchmark"]).status_checked()?;
    git(checkout, ["config", "user.email", "leaven@example.invalid"]).status_checked()?;
    let mutation = mutation_marker(step);
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(checkout.join("src/file-00000.dat"))?;
    file.write_all(mutation.as_bytes())?;
    git(checkout, ["add", "-A"]).status_checked()?;
    git(checkout, ["commit", "-m"])
        .arg(format!("leaven workspace snapshot {step:04}"))
        .status_checked()?;
    let child = git_output(checkout, ["rev-parse", "HEAD"])?
        .trim()
        .to_owned();
    let bundle = checkout.join(".git/leaven-readback.bundle");
    git(checkout, ["bundle", "create"])
        .arg(&bundle)
        .arg("HEAD")
        .arg(format!("^{parent}"))
        .status_checked()?;
    import_bundle(durable, &bundle, parent, &child)?;
    fs::remove_file(bundle)?;
    Ok(ImportedStep {
        commit: child,
        save_seconds: seconds(started.elapsed()),
        changed_bytes: mutation.len(),
    })
}

fn run_intermediate_chain(
    durable: &Path,
    checkout: &Path,
    workspace: &Path,
    parent: &str,
    count: usize,
) -> Result<(String, Option<IntermediateChainReport>)> {
    let durable_before_kib = du_kib(durable)?;
    let mut current_parent = parent.to_owned();
    let mut imported = Vec::with_capacity(count);
    for step in 1..=count {
        let child = mutate_and_import_child_step(durable, checkout, &current_parent, step)?;
        current_parent.clone_from(&child.commit);
        imported.push(child);
    }
    let durable_after_kib = du_kib(durable)?;
    let restore_root = workspace.join("reconstructed");
    fs::create_dir_all(&restore_root)?;
    let mut revisions = Vec::with_capacity(imported.len());
    for (offset, child) in imported.iter().enumerate() {
        let index = offset + 1;
        let restore_workspace = restore_root.join(format!("rev-{index:04}"));
        let started = Instant::now();
        let restored_checkout = materialize_program(durable, &restore_workspace, &child.commit)?;
        let restore_seconds = seconds(started.elapsed());
        let restored_head = git_output(&restored_checkout, ["rev-parse", "HEAD"])?
            .trim()
            .to_owned();
        let restored_content = fs::read(restored_checkout.join("src/file-00000.dat"))?;
        let restored_content_matches =
            contains_bytes(&restored_content, mutation_marker(index).as_bytes())
                && (index == count
                    || !contains_bytes(&restored_content, mutation_marker(index + 1).as_bytes()));
        revisions.push(IntermediateRevisionReport {
            index,
            commit: child.commit.clone(),
            save_seconds: child.save_seconds,
            restore_seconds,
            changed_bytes: child.changed_bytes,
            restored_head_matches: restored_head == child.commit,
            restored_content_matches,
        });
    }
    let changed_bytes = revisions
        .iter()
        .map(|revision| revision.changed_bytes)
        .sum::<usize>();
    let durable_growth_kib = durable_after_kib.saturating_sub(durable_before_kib);
    let storage_amplification = if changed_bytes == 0 {
        0.0
    } else {
        (u64_to_f64(durable_growth_kib) * 1024.0) / usize_to_f64(changed_bytes)
    };
    let save_total_seconds = revisions
        .iter()
        .map(|revision| revision.save_seconds)
        .sum::<f64>();
    let restore_total_seconds = revisions
        .iter()
        .map(|revision| revision.restore_seconds)
        .sum::<f64>();
    let restore_max_seconds = revisions
        .iter()
        .map(|revision| revision.restore_seconds)
        .fold(0.0, f64::max);
    let last_child = current_parent;
    Ok((
        last_child,
        Some(IntermediateChainReport {
            count,
            save_total_seconds,
            save_mean_seconds: mean(revisions.iter().map(|revision| revision.save_seconds)),
            restore_total_seconds,
            restore_mean_seconds: mean(revisions.iter().map(|revision| revision.restore_seconds)),
            restore_max_seconds,
            changed_bytes,
            durable_before_kib,
            durable_after_kib,
            durable_growth_kib,
            storage_amplification,
            revisions,
        }),
    ))
}

fn mutation_marker(step: usize) -> String {
    format!("\nleaven benchmark child mutation step {step:04}\n")
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|candidate| candidate == needle)
}

fn import_bundle(durable: &Path, bundle: &Path, parent: &str, child: &str) -> Result<()> {
    let temp = std::env::temp_dir().join(format!(
        "leaven-git-bundle-{}",
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::create_dir_all(&temp)?;
    git_no_cwd(["init", "--bare"]).arg(&temp).status_checked()?;
    git(&temp, ["fetch"])
        .arg(durable)
        .arg(format!("+{parent}:refs/leaven/parents/{parent}"))
        .status_checked()?;
    git(&temp, ["bundle", "verify"])
        .arg(bundle)
        .status_checked()?;
    git(&temp, ["fetch"])
        .arg(bundle)
        .arg(format!("+{child}:refs/leaven/proposals/{child}"))
        .status_checked()?;
    git(&temp, ["fsck", "--strict"]).status_checked()?;
    git(&temp, ["merge-base", "--is-ancestor", parent, child]).status_checked()?;
    git(durable, ["fetch"])
        .arg(&temp)
        .arg(format!("+{child}:refs/leaven/imported/{child}"))
        .status_checked()?;
    git(durable, ["fsck", "--strict"]).status_checked()?;
    fs::remove_dir_all(temp)?;
    Ok(())
}

fn write_payload(path: &Path, size: usize, case_name: &str, index: usize) -> Result<()> {
    let mut rng = seed(case_name, index);
    let mut file = fs::File::create(path)?;
    let mut remaining = size;
    let mut buffer = [0_u8; 4096];
    while remaining > 0 {
        for byte in &mut buffer {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            *byte = (rng & 0xff) as u8;
        }
        let take = remaining.min(buffer.len());
        file.write_all(&buffer[..take])?;
        remaining -= take;
    }
    Ok(())
}

fn seed(case_name: &str, index: usize) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in case_name.bytes().chain(index.to_le_bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash.max(1)
}

fn summarize(reports: &[SampleReport]) -> BTreeMap<String, SummaryReport> {
    let mut grouped: BTreeMap<String, Vec<&SampleReport>> = BTreeMap::new();
    for report in reports {
        grouped
            .entry(report.case.name.clone())
            .or_default()
            .push(report);
    }
    grouped
        .into_iter()
        .map(|(name, reports)| {
            let first = reports[0];
            (
                name,
                SummaryReport {
                    file_count: first.case.file_count,
                    file_bytes: first.case.file_bytes,
                    total_mib: usize_to_f64(first.case.total_bytes()) / (1024.0 * 1024.0),
                    setup_mean_seconds: mean(reports.iter().map(|report| report.setup_seconds)),
                    projection_mean_seconds: mean(
                        reports.iter().map(|report| report.projection_seconds),
                    ),
                    materialize_mean_seconds: mean(
                        reports.iter().map(|report| report.materialize_seconds),
                    ),
                    readback_mean_seconds: mean(
                        reports.iter().map(|report| report.readback_seconds),
                    ),
                    durable_kib_mean: mean(
                        reports.iter().map(|report| u64_to_f64(report.durable_kib)),
                    ),
                    workspace_kib_mean: mean(
                        reports
                            .iter()
                            .map(|report| u64_to_f64(report.workspace_kib)),
                    ),
                },
            )
        })
        .collect()
}

fn print_summary(summary: &BTreeMap<String, SummaryReport>) {
    println!("summary:");
    println!(
        "case,total_mib,project_mean_s,materialize_mean_s,readback_mean_s,durable_kib,workspace_kib"
    );
    for (name, report) in summary {
        println!(
            "{name},{:.2},{:.3},{:.3},{:.3},{:.0},{:.0}",
            report.total_mib,
            report.projection_mean_seconds,
            report.materialize_mean_seconds,
            report.readback_mean_seconds,
            report.durable_kib_mean,
            report.workspace_kib_mean
        );
    }
}

fn write_report(path: &Path, report: &BenchReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(report)? + "\n")?;
    Ok(())
}

fn samples(
    cases: Vec<BenchCase>,
    iterations: usize,
    intermediate_count: usize,
) -> Vec<GitTrustSample> {
    cases
        .into_iter()
        .flat_map(|case| {
            (1..=iterations).map(move |iteration| GitTrustSample {
                case: case.clone(),
                iteration,
                intermediate_count,
            })
        })
        .collect()
}

fn default_cases() -> Vec<BenchCase> {
    vec![
        BenchCase {
            name: "small".to_owned(),
            file_count: 100,
            file_bytes: 1024,
        },
        BenchCase {
            name: "medium".to_owned(),
            file_count: 1000,
            file_bytes: 4096,
        },
        BenchCase {
            name: "large".to_owned(),
            file_count: 5000,
            file_bytes: 4096,
        },
    ]
}

fn parse_case(raw: &str) -> std::result::Result<BenchCase, String> {
    let mut parts = raw.split(':');
    let name = parts
        .next()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "case name is required".to_owned())?;
    let file_count = parts
        .next()
        .ok_or_else(|| "file count is required".to_owned())?
        .parse::<usize>()
        .map_err(|source| format!("invalid file count: {source}"))?;
    let file_bytes = parts
        .next()
        .ok_or_else(|| "file bytes is required".to_owned())?
        .parse::<usize>()
        .map_err(|source| format!("invalid file bytes: {source}"))?;
    if parts.next().is_some() {
        return Err("case must be NAME:FILES:BYTES".to_owned());
    }
    if file_count == 0 || file_bytes == 0 {
        return Err("file count and bytes must be positive".to_owned());
    }
    Ok(BenchCase {
        name: name.to_owned(),
        file_count,
        file_bytes,
    })
}

fn git<const N: usize>(cwd: &Path, args: [&str; N]) -> GitCommand {
    let mut command = Command::new("git");
    command.current_dir(cwd).args(args);
    GitCommand(command)
}

fn git_no_cwd<const N: usize>(args: [&str; N]) -> GitCommand {
    let mut command = Command::new("git");
    command.args(args);
    GitCommand(command)
}

fn git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<String> {
    let output = git(cwd, args).output_checked()?;
    Ok(String::from_utf8(output.stdout)?)
}

fn git_success<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<bool> {
    Ok(git(cwd, args).output()?.status.success())
}

struct GitCommand(Command);

impl GitCommand {
    fn arg<T: AsRef<OsStr>>(&mut self, arg: T) -> &mut Self {
        self.0.arg(arg);
        self
    }

    fn status_checked(&mut self) -> Result<()> {
        let output = self.output_checked()?;
        drop(output);
        Ok(())
    }

    fn output(&mut self) -> Result<Output> {
        Ok(self.with_envs().output()?)
    }

    fn output_checked(&mut self) -> Result<Output> {
        let output = self.output()?;
        if !output.status.success() {
            return Err(format!(
                "command {:?} failed: {}",
                self.0,
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        Ok(output)
    }

    fn with_envs(&mut self) -> &mut Command {
        self.0
            .env("GIT_AUTHOR_NAME", "Leaven Benchmark")
            .env("GIT_AUTHOR_EMAIL", "leaven@example.invalid")
            .env("GIT_COMMITTER_NAME", "Leaven Benchmark")
            .env("GIT_COMMITTER_EMAIL", "leaven@example.invalid")
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
    }
}

fn run_command(command: &mut Command, verbose: bool) -> Result<()> {
    if verbose {
        println!("running: {command:?}");
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "command {command:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

fn du_kib(path: &Path) -> Result<u64> {
    let output = Command::new("du").arg("-sk").arg(path).output()?;
    if !output.status.success() {
        return Err(format!("du failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    let first = stdout
        .split_whitespace()
        .next()
        .ok_or("du output was empty")?;
    Ok(first.parse()?)
}

fn repo_root() -> Result<PathBuf> {
    for mut command in [
        {
            let mut command = Command::new("jj");
            command.arg("root");
            command
        },
        {
            let mut command = Command::new("git");
            command.args(["rev-parse", "--show-toplevel"]);
            command
        },
    ] {
        let Ok(output) = command.output() else {
            continue;
        };
        if output.status.success() {
            return Ok(PathBuf::from(String::from_utf8(output.stdout)?.trim()));
        }
    }
    Err("could not determine repository root with jj or git".into())
}

fn logical_cpus() -> usize {
    thread::available_parallelism().map_or(1, usize::from)
}

fn half_parallelism() -> usize {
    (logical_cpus() / 2).max(1)
}

fn seconds(duration: Duration) -> f64 {
    duration.as_secs_f64()
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let mut count = 0.0;
    let mut sum = 0.0;
    for value in values {
        count += 1.0;
        sum += value;
    }
    if count == 0.0 { 0.0 } else { sum / count }
}

fn usize_to_f64(value: usize) -> f64 {
    value
        .to_string()
        .parse()
        .expect("usize decimal representation parses as f64")
}

fn u64_to_f64(value: u64) -> f64 {
    value
        .to_string()
        .parse()
        .expect("u64 decimal representation parses as f64")
}
