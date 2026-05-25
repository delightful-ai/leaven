use std::{collections::BTreeMap, fs, path::Path};

use serde::Serialize;

use super::{
    BenchCase, EnvironmentKind, IntermediateChainReport, Result, TrustScore,
    util::{mean, u64_to_f64, usize_to_f64},
};

#[derive(Debug, Serialize)]
pub(super) struct BenchReport {
    pub(super) generated_at: String,
    pub(super) task: TaskReport,
    pub(super) host: HostReport,
    pub(super) commands: Vec<CommandReport>,
    pub(super) samples: Vec<SampleReport>,
    pub(super) summary: BTreeMap<String, SummaryReport>,
    pub(super) resource_usage: ResourceReport,
}

#[derive(Debug, Serialize)]
pub(super) struct TaskReport {
    pub(super) name: &'static str,
    pub(super) structure: &'static str,
    pub(super) environment: EnvironmentKind,
    pub(super) sample_count: usize,
    pub(super) intermediate_count: usize,
}

#[derive(Debug, Serialize)]
pub(super) struct HostReport {
    pub(super) logical_cpus: usize,
    pub(super) jobs: usize,
    pub(super) os: String,
    pub(super) arch: String,
}

#[derive(Debug, Serialize)]
pub(super) struct CommandReport {
    pub(super) command: Vec<String>,
    pub(super) seconds: f64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct SampleReport {
    pub(super) case: BenchCase,
    pub(super) iteration: usize,
    pub(super) setup_seconds: f64,
    pub(super) projection_seconds: f64,
    pub(super) materialize_seconds: f64,
    pub(super) readback_seconds: f64,
    pub(super) durable_kib: u64,
    pub(super) workspace_kib: u64,
    pub(super) imported_child: String,
    pub(super) intermediate_chain: Option<IntermediateChainReport>,
    pub(super) score: TrustScore,
}

#[derive(Debug, Serialize)]
pub(super) struct SummaryReport {
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
pub(super) struct ResourceReport {
    pub(super) wall_seconds: f64,
}

pub(super) fn summarize(reports: &[SampleReport]) -> BTreeMap<String, SummaryReport> {
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

pub(super) fn print_summary(summary: &BTreeMap<String, SummaryReport>) {
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

pub(super) fn write_report(path: &Path, report: &BenchReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(report)? + "\n")?;
    Ok(())
}
