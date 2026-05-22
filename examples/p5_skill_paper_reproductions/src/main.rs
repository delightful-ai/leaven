use std::path::PathBuf;

use clap::Parser;
use p5_skill_paper_reproductions::evoskill::{ManifestBuildInput, build_evoskill_replica_manifest};

#[derive(Debug, Parser)]
struct Args {
    /// Repository root to inspect for local paper/source artifacts.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Write the generated `EvoSkill` replica manifest to this path.
    #[arg(long)]
    out: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let manifest = build_evoskill_replica_manifest(&ManifestBuildInput::new(args.root))?;
    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(args.out, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(())
}
