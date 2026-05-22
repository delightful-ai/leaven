use std::path::PathBuf;

use clap::Parser;
use p5_skill_paper_reproductions::evoskill::{
    ManifestBuildInput, build_evoskill_final_report, build_evoskill_replica_manifest,
    write_evoskill_browsecomp_public_transfer_sample, write_evoskill_local_source_pin_manifest,
    write_evoskill_officeqa_score_result_manifest,
    write_evoskill_paper_close_split_policy_manifest,
    write_evoskill_sealqa_judge_score_result_manifest,
};

#[derive(Debug, Parser)]
struct Args {
    /// Repository root to inspect for local paper/source artifacts.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Write the generated `EvoSkill` replica manifest to this path.
    #[arg(long)]
    out: PathBuf,
    /// Also write the current no-spend final report truth surface to this path.
    #[arg(long)]
    final_report_out: Option<PathBuf>,
    /// Persist the current local git source checkouts as the explicit source pin sidecar.
    #[arg(long)]
    write_local_source_pin_manifest: bool,
    /// Persist current substitute split fingerprints as the explicit paper-close split policy.
    #[arg(long)]
    write_paper_close_split_policy_manifest: bool,
    /// Derive the strict 128-row `BrowseComp` transfer sidecar from a local official public CSV.
    #[arg(long)]
    write_browsecomp_public_transfer_sample: Option<PathBuf>,
    /// Score `OfficeQA` prediction JSONL rows with the Rust scorer and write the strict score sidecar.
    #[arg(long)]
    write_officeqa_score_result: Option<PathBuf>,
    /// Import approved `SealQA` external-judge JSONL rows and write the strict score sidecar.
    #[arg(long)]
    write_sealqa_judge_score_result: Option<PathBuf>,
    /// Approval/provenance id for the imported `SealQA` external judge run.
    #[arg(long)]
    sealqa_judge_approval_id: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let input = ManifestBuildInput::new(args.root);
    if args.write_local_source_pin_manifest {
        write_evoskill_local_source_pin_manifest(&input)?;
    }
    if let Some(path) = args.write_browsecomp_public_transfer_sample {
        write_evoskill_browsecomp_public_transfer_sample(&input, path)?;
    }
    if args.write_paper_close_split_policy_manifest {
        write_evoskill_paper_close_split_policy_manifest(&input)?;
    }
    if let Some(path) = args.write_officeqa_score_result {
        write_evoskill_officeqa_score_result_manifest(&input, path)?;
    }
    if let Some(path) = args.write_sealqa_judge_score_result {
        let approval_id = args
            .sealqa_judge_approval_id
            .as_deref()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "SealQA judge score writer requires a nonempty approval id via --sealqa-judge-approval-id",
                )
            })?;
        write_evoskill_sealqa_judge_score_result_manifest(&input, path, approval_id)?;
    }
    let manifest = build_evoskill_replica_manifest(&input)?;
    write_json(&args.out, &manifest)?;
    if let Some(path) = args.final_report_out {
        let report = build_evoskill_final_report(&input)?;
        write_json(&path, &report)?;
    }
    Ok(())
}

fn write_json(
    path: &PathBuf,
    value: &impl serde::Serialize,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
