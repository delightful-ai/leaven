//! Public-surface contract for the `leaven` umbrella crate.
//!
//! The umbrella routes every individually re-exported symbol through exactly
//! one audience-named module: `prelude` (ordinary users), `extend` (users
//! implementing a piece of the machine), or `plumbing` (`#[doc(hidden)]`,
//! cross-crate/test reach only). This test is the mechanical gate that keeps
//! that routing honest, so internal plumbing cannot drift back into the
//! ordinary import experience.
//!
//! It enforces five properties:
//!
//! 1. Every symbol re-exported by a route module appears in `SURFACE` exactly
//!    once.
//! 2. Every `SURFACE` entry is still re-exported by the source.
//! 3. Each route module re-exports only symbols whose `SURFACE` route matches.
//! 4. Every `Extend` and `Plumbing` entry carries a non-empty `reason` naming
//!    the concrete consumer that forces the symbol to be public.
//! 5. `lib.rs` re-exports no individual type at the crate root: it carries
//!    only `pub mod` route modules, crate aliases, and feature-gated module
//!    re-exports. The route module *is* the classification.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Audience route a re-exported umbrella symbol belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    /// Ordinary users: define a problem, call `optimize`, write a scorer.
    Prelude,
    /// Users implementing a piece of the machine: optimizer, proposer, store.
    Extend,
    /// `#[doc(hidden)]`: public only for sibling-crate or contract-test reach.
    Plumbing,
}

/// The checked registry of every symbol routed by the `leaven` umbrella.
///
/// `reason` is the justification naming the concrete consumer; it must be
/// non-empty for every `Extend` and `Plumbing` entry. `Prelude` entries are
/// ordinary product vocabulary and use an empty reason.
const SURFACE: &[(&str, Route, &str)] = &[
    // --- prelude: ordinary users. ---
    ("Artifact", Route::Prelude, ""),
    ("ArtifactIdentity", Route::Prelude, ""),
    ("OptimizationProblem", Route::Prelude, ""),
    ("Assessment", Route::Prelude, ""),
    ("AssessmentGranularity", Route::Prelude, ""),
    ("AssessmentTarget", Route::Prelude, ""),
    ("Evidence", Route::Prelude, ""),
    ("PairOrder", Route::Prelude, ""),
    ("Preference", Route::Prelude, ""),
    ("EditSurface", Route::Prelude, ""),
    ("Part", Route::Prelude, ""),
    ("PartAddress", Route::Prelude, ""),
    ("PartSelection", Route::Prelude, ""),
    ("SurfaceError", Route::Prelude, ""),
    ("SurfaceFingerprint", Route::Prelude, ""),
    ("Budget", Route::Prelude, ""),
    ("CandidateId", Route::Prelude, ""),
    ("Cost", Route::Prelude, ""),
    ("CostUnit", Route::Prelude, ""),
    ("BestCandidate", Route::Prelude, ""),
    ("Optimized", Route::Prelude, ""),
    ("OptimizeBuilder", Route::Prelude, ""),
    ("OptimizeError", Route::Prelude, ""),
    ("RunEventSummary", Route::Prelude, ""),
    ("RunError", Route::Prelude, ""),
    ("RunOutput", Route::Prelude, ""),
    ("Score", Route::Prelude, ""),
    ("ScoreContext", Route::Prelude, ""),
    ("ScoreError", Route::Prelude, ""),
    ("StandardRunSummary", Route::Prelude, ""),
    ("optimize", Route::Prelude, ""),
    ("DeriveArtifact", Route::Prelude, ""),
    ("DeriveContentAddressed", Route::Prelude, ""),
    ("DeriveEditSurface", Route::Prelude, ""),
    // --- extend: engine stage traits implemented by component authors. ---
    (
        "Optimizer",
        Route::Extend,
        "custom optimizer authors implement it",
    ),
    (
        "Proposer",
        Route::Extend,
        "custom proposer authors implement it",
    ),
    (
        "Evaluator",
        Route::Extend,
        "custom evaluator authors implement it",
    ),
    (
        "Materializer",
        Route::Extend,
        "custom materializer authors implement it",
    ),
    (
        "Renderer",
        Route::Extend,
        "custom renderer authors implement it",
    ),
    (
        "Stopper",
        Route::Extend,
        "custom stop-condition authors implement it",
    ),
    (
        "Population",
        Route::Extend,
        "population authors implement it",
    ),
    (
        "PreferenceRelation",
        Route::Extend,
        "selection-policy authors implement it",
    ),
    // --- extend: engine drivers and run inspection. ---
    (
        "Engine",
        Route::Extend,
        "component test harnesses drive a run",
    ),
    (
        "EngineBuilder",
        Route::Extend,
        "component test harnesses configure runs",
    ),
    (
        "RunResult",
        Route::Extend,
        "harness code inspects completed runs",
    ),
    (
        "RunEvent",
        Route::Extend,
        "callback and harness authors observe runs",
    ),
    (
        "StepStatus",
        Route::Extend,
        "callback and harness authors branch on step outcome",
    ),
    (
        "Arity",
        Route::Extend,
        "proposer authors declare candidate arity",
    ),
    // --- extend: stage contexts. ---
    (
        "RunContext",
        Route::Extend,
        "stage authors mutate the run graph",
    ),
    (
        "RunGraphView",
        Route::Extend,
        "stage authors read the run graph projection",
    ),
    (
        "ProposalContext",
        Route::Extend,
        "Proposer::propose receives it",
    ),
    (
        "RenderContext",
        Route::Extend,
        "Renderer::render receives it",
    ),
    (
        "MaterializeContext",
        Route::Extend,
        "Materializer::materialize receives it",
    ),
    (
        "MaterializationReport",
        Route::Extend,
        "Materializer authors return it",
    ),
    (
        "MaterializeError",
        Route::Extend,
        "Materializer authors return it on failure",
    ),
    // --- extend: trust, scope, and cache policy. ---
    (
        "TrustPolicy",
        Route::Extend,
        "stage authors declare read trust",
    ),
    (
        "ReadScope",
        Route::Extend,
        "stage authors bound observed graph nodes",
    ),
    (
        "CachePolicy",
        Route::Extend,
        "Evaluator authors declare cacheability",
    ),
    // --- extend: cold algebra a stage author emits. ---
    ("InfoRef", Route::Extend, "proposer authors attach lineage"),
    (
        "CausalInputs",
        Route::Extend,
        "proposer authors record causal inputs",
    ),
    (
        "Proposal",
        Route::Extend,
        "proposer authors build candidate actions",
    ),
    (
        "ProposalBatch",
        Route::Extend,
        "Proposer::propose returns a sibling batch",
    ),
    (
        "ProposalProvenance",
        Route::Extend,
        "proposer authors record how a proposal was made",
    ),
    (
        "ProposalEffect",
        Route::Extend,
        "proposer authors choose Create versus Change",
    ),
    (
        "ProposalBatchSemantics",
        Route::Extend,
        "proposer authors declare batch combination rules",
    ),
    (
        "EvaluationRequest",
        Route::Extend,
        "evaluator authors read which candidates to score",
    ),
    (
        "EvaluationSet",
        Route::Extend,
        "evaluator authors read which cases a request spans",
    ),
    (
        "PartitionId",
        Route::Extend,
        "evaluator and dataset authors partition cases",
    ),
    // --- extend: run extension surface. ---
    (
        "OptimizeStore",
        Route::Extend,
        "custom store authors implement it",
    ),
    (
        "IntoOptimizeStore",
        Route::Extend,
        "store wiring authors implement it",
    ),
    // --- extend: LM provider vocabulary. ---
    (
        "Lm",
        Route::Extend,
        "LM provider authors implement the trait",
    ),
    ("LmRequest", Route::Extend, "LM provider authors accept it"),
    ("LmResponse", Route::Extend, "LM provider authors return it"),
    (
        "LmContinuation",
        Route::Extend,
        "LM provider authors drive multi-turn",
    ),
    (
        "LmError",
        Route::Extend,
        "LM provider authors return it on failure",
    ),
    (
        "LmId",
        Route::Extend,
        "LM provider authors identify an instance",
    ),
    (
        "Message",
        Route::Extend,
        "LM provider authors build request turns",
    ),
    (
        "Messages",
        Route::Extend,
        "LM provider authors build transcripts",
    ),
    (
        "ModelName",
        Route::Extend,
        "LM provider authors map to a model",
    ),
    (
        "ModelRole",
        Route::Extend,
        "LM provider authors preserve policy/routing roles",
    ),
    (
        "OutputMode",
        Route::Extend,
        "LM provider authors shape the response",
    ),
    (
        "ProviderHints",
        Route::Extend,
        "LM provider authors read provider knobs",
    ),
    (
        "ProviderName",
        Route::Extend,
        "LM provider authors identify the family",
    ),
    (
        "ReasoningEffort",
        Route::Extend,
        "LM provider authors set reasoning budget",
    ),
    (
        "Role",
        Route::Extend,
        "LM provider authors tag each message",
    ),
    (
        "SamplingOptions",
        Route::Extend,
        "LM provider authors honor sampling knobs",
    ),
    (
        "TokenUsage",
        Route::Extend,
        "LM provider authors report metered cost",
    ),
    // --- plumbing: doc(hidden), cross-crate and test reach only. ---
    (
        "ContentAddressed",
        Route::Plumbing,
        "derive-macro output and content-store internals",
    ),
    (
        "CacheIdentity",
        Route::Plumbing,
        "engine cache-key internals",
    ),
    (
        "ContentId",
        Route::Plumbing,
        "content-store and fingerprint internals",
    ),
    (
        "Fingerprint",
        Route::Plumbing,
        "surface and engine fingerprint plumbing",
    ),
    (
        "Amount",
        Route::Plumbing,
        "cost and score arithmetic internals",
    ),
    (
        "AmountError",
        Route::Plumbing,
        "cost arithmetic overflow internals",
    ),
    (
        "BudgetSnapshot",
        Route::Plumbing,
        "engine budget-ledger snapshot internals",
    ),
    (
        "FiniteF64",
        Route::Plumbing,
        "score and cost finite-number internals",
    ),
    (
        "FiniteF64Error",
        Route::Plumbing,
        "finite-number construction internals",
    ),
    (
        "ErrorRecord",
        Route::Plumbing,
        "durable error serialization plumbing",
    ),
    (
        "MetadataBag",
        Route::Plumbing,
        "graph and evidence metadata plumbing",
    ),
    (
        "ProposalId",
        Route::Plumbing,
        "engine graph proposal-node plumbing",
    ),
];

#[test]
fn every_routed_symbol_is_registered_exactly_once() {
    let registry = registry_symbols();
    assert_eq!(
        registry.len(),
        SURFACE.len(),
        "SURFACE has a duplicate symbol entry"
    );

    for (route, file) in route_files() {
        for symbol in reexported_symbols(&file) {
            let entry = SURFACE.iter().find(|(name, _, _)| *name == symbol);
            let Some((_, registered_route, _)) = entry else {
                panic!(
                    "new public symbol `{symbol}` re-exported by {} is unclassified -- \
                     add it to SURFACE in public_surface_contract.rs with route + reason, \
                     or make it pub(crate)",
                    file.display()
                );
            };
            assert_eq!(
                *registered_route, route,
                "`{symbol}` is re-exported by the {route:?} route but SURFACE classifies \
                 it as {registered_route:?}: move the re-export or fix the SURFACE route"
            );
        }
    }
}

#[test]
fn every_registered_symbol_is_still_reexported() {
    let mut routed = BTreeSet::new();
    for (_, file) in route_files() {
        routed.extend(reexported_symbols(&file));
    }
    for (symbol, _, _) in SURFACE {
        assert!(
            routed.contains(*symbol),
            "SURFACE lists `{symbol}` but no route module re-exports it -- \
             remove the stale SURFACE entry or restore the re-export"
        );
    }
}

#[test]
fn extend_and_plumbing_entries_name_a_consumer() {
    for (symbol, route, reason) in SURFACE {
        match route {
            Route::Prelude => {}
            Route::Extend | Route::Plumbing => assert!(
                !reason.trim().is_empty(),
                "`{symbol}` is routed as {route:?} but has an empty reason -- \
                 name the concrete consumer that needs it, or make it pub(crate)"
            ),
        }
    }
}

#[cfg(feature = "gepa")]
#[test]
fn gepa_route_exposes_typed_report_extension_without_prelude_pollution() {
    fn assert_gepa_ext<T>()
    where
        T: leaven::gepa::GepaOptimizedExt,
    {
    }

    assert_gepa_ext::<leaven::prelude::Optimized<()>>();
}

#[test]
fn lib_rs_routes_no_individual_symbol_at_the_crate_root() {
    let lib = fs::read_to_string(umbrella_src().join("lib.rs")).unwrap();
    for raw in lib.lines() {
        let line = raw.split("//").next().unwrap_or("").trim();
        let Some(rest) = line.strip_prefix("pub use ") else {
            continue;
        };
        assert!(
            !rest.contains('{'),
            "lib.rs re-exports individual symbols at the crate root with `{line}` -- \
             every individual symbol must live inside the prelude, extend, or plumbing \
             route module"
        );
        let item = rest.trim_end_matches(';').trim();
        // A crate alias (`pub use leaven_core as core;`) is the only allowed
        // `pub use`; an individual symbol re-export (`pub use ...::Foo;`) is not.
        assert!(
            item.contains(" as "),
            "lib.rs re-exports `{item}` at the crate root -- only `pub use crate as alias` \
             is allowed; route the symbol through prelude, extend, or plumbing"
        );
    }
}

/// The route modules paired with the route they own.
fn route_files() -> Vec<(Route, PathBuf)> {
    let src = umbrella_src();
    vec![
        (Route::Prelude, src.join("prelude.rs")),
        (Route::Extend, src.join("extend.rs")),
        (Route::Plumbing, src.join("plumbing.rs")),
    ]
}

/// Final exported names from every `pub use` in a route module.
///
/// Handles `pub use path::Symbol;` and braced `pub use path::{A, B as C};`,
/// across line breaks, and resolves `X as Y` to the surfaced name `Y`.
fn reexported_symbols(file: &Path) -> BTreeSet<String> {
    let text = fs::read_to_string(file)
        .unwrap_or_else(|err| panic!("route module {} must exist: {err}", file.display()));
    let mut symbols = BTreeSet::new();
    let mut rest = text.as_str();
    while let Some(start) = rest.find("pub use ") {
        let after = &rest[start + "pub use ".len()..];
        let end = after.find(';').expect("pub use statement must end with ;");
        let statement = &after[..end];
        rest = &after[end + 1..];
        let names = match statement.split_once('{') {
            Some((_, braced)) => braced.trim_end_matches('}'),
            None => statement.rsplit("::").next().unwrap_or(statement),
        };
        for raw in names.split(',') {
            let token = raw.trim();
            if token.is_empty() {
                continue;
            }
            let surfaced = match token.split_once(" as ") {
                Some((_, alias)) => alias.trim(),
                None => token.rsplit("::").next().unwrap_or(token).trim(),
            };
            symbols.insert(surfaced.to_owned());
        }
    }
    symbols
}

fn registry_symbols() -> BTreeSet<String> {
    SURFACE
        .iter()
        .map(|(name, _, _)| (*name).to_owned())
        .collect()
}

fn umbrella_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}
