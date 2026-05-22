use std::path::PathBuf;

use clap::Parser;
use p5_skill_paper_reproductions::evoskill::{
    ManifestBuildInput, build_evoskill_final_report, build_evoskill_replica_manifest,
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let input = ManifestBuildInput::new(args.root);
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
