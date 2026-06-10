use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use leaven_engine::{Callback, RunEvent, RunGraphView};
use leaven_gepa::{GepaEventSummary, GepaReport};
use leaven_kernel::CandidateId;
use leaven_run::RunProblem;
use serde_json::{Value, json};

use super::problem::SeamPromptArtifact;

type SeamProblem = RunProblem<SeamPromptArtifact, Value, Value>;

/// Snapshot of every candidate template observed in the run graph.
///
/// The `Optimized` result only carries the best artifact, but the result
/// projection needs every frontier candidate's template to re-encode the wire
/// artifact triple. This callback snapshots each candidate's template from
/// graph truth whenever a candidate is created or applied, keyed by graph
/// `CandidateId`.
#[derive(Clone, Default)]
pub(super) struct CandidateArtifacts {
    templates: Arc<Mutex<BTreeMap<CandidateId, String>>>,
}

impl CandidateArtifacts {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn template(&self, candidate: CandidateId) -> Option<String> {
        self.templates
            .lock()
            .expect("candidate artifact snapshot lock poisoned")
            .get(&candidate)
            .cloned()
    }

    fn snapshot(&self, candidate: CandidateId, view: &RunGraphView<'_, SeamProblem>) {
        if let Some(artifact) = view.artifact(candidate) {
            self.templates
                .lock()
                .expect("candidate artifact snapshot lock poisoned")
                .insert(candidate, artifact.template().to_owned());
        }
    }

    fn snapshot_all(&self, view: &RunGraphView<'_, SeamProblem>) {
        let mut templates = self
            .templates
            .lock()
            .expect("candidate artifact snapshot lock poisoned");
        for candidate in view.candidate_tree().roots() {
            collect_subtree(view, candidate, &mut templates);
        }
    }
}

fn collect_subtree(
    view: &RunGraphView<'_, SeamProblem>,
    candidate: CandidateId,
    templates: &mut BTreeMap<CandidateId, String>,
) {
    if let Some(artifact) = view.artifact(candidate) {
        templates.insert(candidate, artifact.template().to_owned());
    }
    for child in view.children(candidate) {
        collect_subtree(view, child, templates);
    }
}

/// Engine callback that snapshots candidate templates from graph truth.
pub(super) struct CandidateArtifactSnapshot {
    artifacts: CandidateArtifacts,
}

impl CandidateArtifactSnapshot {
    pub(super) fn new(artifacts: CandidateArtifacts) -> Self {
        Self { artifacts }
    }
}

impl Callback<SeamProblem> for CandidateArtifactSnapshot {
    fn on_event(&mut self, event: &RunEvent, graph: RunGraphView<'_, SeamProblem>) {
        match event {
            RunEvent::ApplySucceeded { candidate_id, .. } => {
                self.artifacts.snapshot(*candidate_id, &graph);
            }
            RunEvent::OptimizationStarted { .. } | RunEvent::OptimizationEnded { .. } => {
                self.artifacts.snapshot_all(&graph);
            }
            _ => {}
        }
    }
}

/// GEPA event sink: records every event summary and mirrors one structured line
/// per event to stderr for live visibility (wire streaming stays deferred).
#[derive(Clone, Default)]
pub(super) struct GepaEventLog {
    events: Arc<Mutex<Vec<GepaEventSummary>>>,
}

impl GepaEventLog {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn record(&self, event: &GepaEventSummary) {
        eprintln!(
            "optimize_run_event={}",
            serde_json::to_string(event)
                .unwrap_or_else(|_| "{\"error\":\"event serialize\"}".to_owned())
        );
        self.events
            .lock()
            .expect("gepa event log lock poisoned")
            .push(event.clone());
    }

    pub(super) fn snapshot(&self) -> Vec<GepaEventSummary> {
        self.events
            .lock()
            .expect("gepa event log lock poisoned")
            .clone()
    }
}

/// Writes the GEPA event summaries and final report into the run directory,
/// mirroring p8's report-file pattern.
pub(super) fn write_run_instrumentation(
    run_dir: &Path,
    events: &[GepaEventSummary],
    report: Option<&GepaReport>,
) {
    let payload = json!({
        "schema": "leaven.optimize_run.instrumentation.v1",
        "gepa_events": events,
        "gepa_report": report,
    });
    let Ok(bytes) = serde_json::to_vec_pretty(&payload) else {
        return;
    };
    let path = run_dir.join("optimize_run_instrumentation.json");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = write_atomic(&path, &bytes);
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}
