use std::collections::BTreeSet;

const PLACEHOLDER_PRELUDE_EXPORTS: &[&str] = &[
    "DiffEvidence",
    "RenderedDiff",
    "JsonEvidence",
    "ListwiseRankingEvidence",
    "RankingItem",
    "MixedEvidence",
    "Direction",
    "RawScoreValue",
    "ScoreAxis",
    "ScorePoint",
    "ScoreVectorEvidence",
    "StringEvidence",
];

#[test]
fn prelude_does_not_export_reserved_placeholder_evidence_names() {
    let lib = std::fs::read_to_string("src/lib.rs").expect("read evidence crate root");
    let prelude = prelude_reexports(&lib);

    for symbol in PLACEHOLDER_PRELUDE_EXPORTS {
        assert!(
            !prelude.contains(*symbol),
            "`{symbol}` is reserved scaffold and must stay out of leaven_evidence::prelude::*"
        );
    }
}

#[test]
fn crate_root_does_not_reserve_empty_placeholder_evidence_names() {
    let lib = std::fs::read_to_string("src/lib.rs").expect("read evidence crate root");

    for symbol in PLACEHOLDER_PRELUDE_EXPORTS {
        assert!(
            !lib.contains(symbol),
            "`{symbol}` is inert scaffold; reintroduce it only with behavior and contract tests"
        );
    }
}

fn prelude_reexports(lib: &str) -> BTreeSet<String> {
    let prelude_start = lib
        .find("pub mod prelude {")
        .expect("evidence prelude module exists");
    let prelude = &lib[prelude_start..];
    let use_start = prelude
        .find("pub use crate::{")
        .expect("evidence prelude uses one curated crate list");
    let after_use = &prelude[use_start + "pub use crate::{".len()..];
    let use_end = after_use
        .find("};")
        .expect("evidence prelude use list ends");

    after_use[..use_end]
        .split(',')
        .map(str::trim)
        .filter(|symbol| !symbol.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
