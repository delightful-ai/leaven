use std::error::Error;
use std::fmt;

use leaven_kernel::{
    AgentId, BlobRef, CandidateId, CaseId, CaseRunId, ContentId, ErrorKind, ErrorRecord,
    EvaluatorId, Fingerprint, FingerprintBuilder, IntoErrorRecord, MetadataBag, MetadataKey,
    MetadataValue, ProposerId, RendererId, Retryability, RunId, StageId, StopperId, TraceRef,
};
use uuid::Uuid;

#[test]
fn fingerprints_are_stable_for_ordered_inputs() {
    let mut builder = FingerprintBuilder::new();
    builder.update("stage=").update("eval");
    let fingerprint = builder.finish();
    let mut same_builder = FingerprintBuilder::new();
    same_builder.update("stage=").update("eval");
    let same = same_builder.finish();
    let mut different_order_builder = FingerprintBuilder::new();
    different_order_builder.update("eval").update("stage=");
    let different_order = different_order_builder.finish();

    assert_eq!(fingerprint, same);
    assert_ne!(fingerprint, different_order);
    assert_eq!(Fingerprint::from_bytes(fingerprint.0), fingerprint);
}

#[test]
fn fingerprint_hex_is_full_width_and_lowercase() {
    let fingerprint = Fingerprint::from_bytes([0xab; 32]);

    assert_eq!(
        fingerprint.to_hex(),
        "abababababababababababababababababababababababababababababababab"
    );
}

#[test]
fn metadata_bag_preserves_typed_values_in_key_order() {
    let dynamic = MetadataKey::new("worker");
    let mut bag = MetadataBag::new();

    assert!(bag.is_empty());
    bag.insert(dynamic.clone(), MetadataValue::String("local".to_owned()))
        .insert("attempt", MetadataValue::U64(2))
        .insert(
            MetadataKey::from(String::from("blob")),
            MetadataValue::BlobRef(BlobRef {
                store: "inline".to_owned(),
                key: "trace/1".to_owned(),
            }),
        );

    assert_eq!(dynamic.as_str(), "worker");
    assert_eq!(bag.len(), 3);
    assert!(matches!(
        bag.get(&dynamic),
        Some(MetadataValue::String(value)) if value == "local"
    ));
    assert_eq!(
        bag.iter()
            .map(|(key, _)| key.as_str().to_owned())
            .collect::<Vec<_>>(),
        vec!["attempt", "blob", "worker"]
    );
}

#[test]
fn durable_error_records_capture_sources_and_identity_conversion() {
    let source = ChainError::with_depth(18);
    let record = ErrorRecord::from_error(ErrorKind::Evaluation, &source);
    let plain = ErrorRecord::new(ErrorKind::Trust, "hidden partition");
    let identity = plain.clone().into_error_record();

    assert_eq!(plain.message, "hidden partition");
    assert_eq!(record.kind, ErrorKind::Evaluation);
    assert_eq!(record.message, "layer 18");
    assert_eq!(record.retryability, Retryability::Unknown);
    assert_eq!(record.source_chain.len(), 17);
    assert!(record.debug.as_deref().unwrap().contains("ChainError"));
    assert_eq!(identity.kind, ErrorKind::Trust);
    assert_eq!(identity.message, "hidden partition");
}

#[test]
fn typed_ids_preserve_their_distinct_display_and_raw_forms() {
    let uuid = Uuid::nil();
    let run = RunId::from_uuid(uuid);
    let case_run = CaseRunId::from_uuid(uuid);
    let generated = CandidateId::new();
    let defaulted = CandidateId::default();
    let content = ContentId::from_bytes([9; ContentId::BYTES]);
    let zero = ContentId::zero();
    let case = CaseId::from_index(7);
    let agent = AgentId::from("worker");
    let trace = TraceRef {
        store: "trace-store".to_owned(),
        key: "session/abc".to_owned(),
    };
    let proposer = ProposerId::new(String::from("gepa/reflect"));
    let evaluator = EvaluatorId::PRIMARY;

    assert_eq!(run.as_uuid(), uuid);
    assert_eq!(case_run.as_uuid(), uuid);
    assert_ne!(generated, defaulted);
    assert_eq!(content.as_bytes(), &[9; ContentId::BYTES]);
    assert_eq!(zero.as_bytes(), &[0; ContentId::BYTES]);
    assert_eq!(format!("{content}"), "cid:0909090909090909");
    assert_eq!(format!("{}", CaseId::new(3)), "case:3");
    assert_eq!(format!("{case}"), "case:7");
    assert_eq!(agent.as_str(), "worker");
    assert_eq!(format!("{agent}"), "worker");
    assert_eq!(trace.store, "trace-store");
    assert_eq!(trace.key, "session/abc");
    assert_eq!(proposer.as_str(), "gepa/reflect");
    assert_eq!(format!("{proposer}"), "gepa/reflect");
    assert_eq!(evaluator.as_str(), "primary");
}

#[test]
fn stage_ids_keep_stage_kind_in_display() {
    let proposer = StageId::from_proposer(ProposerId::from("p"));
    let evaluator = StageId::from_evaluator(EvaluatorId::PAIRWISE_JUDGE);
    let renderer = StageId::from_renderer(RendererId::from("materializer"));
    let stopper = StageId::Stopper(StopperId::from("max_iters"));
    let custom = StageId::custom("coverage-audit");

    assert_eq!(format!("{proposer}"), "proposer:p");
    assert_eq!(format!("{evaluator}"), "evaluator:pairwise_judge");
    assert_eq!(format!("{renderer}"), "renderer:materializer");
    assert_eq!(format!("{stopper}"), "stopper:max_iters");
    assert_eq!(format!("{custom}"), "custom:coverage-audit");
}

#[test]
fn stage_ids_are_json_map_keys() {
    use std::collections::BTreeMap;

    let mut stages = BTreeMap::new();
    stages.insert(StageId::from_proposer(ProposerId::from("p")), 1_u64);
    stages.insert(StageId::custom("coverage-audit"), 2_u64);

    let encoded = serde_json::to_string(&stages).unwrap();
    assert!(encoded.contains("\"proposer:p\""));
    assert!(encoded.contains("\"custom:coverage-audit\""));

    let decoded: BTreeMap<StageId, u64> = serde_json::from_str(&encoded).unwrap();

    assert_eq!(
        decoded.get(&StageId::from_proposer(ProposerId::from("p"))),
        Some(&1)
    );
    assert_eq!(decoded.get(&StageId::custom("coverage-audit")), Some(&2));
}

#[derive(Debug)]
struct ChainError {
    layer: usize,
    source: Option<Box<Self>>,
}

impl ChainError {
    fn with_depth(depth: usize) -> Self {
        if depth == 0 {
            Self {
                layer: 0,
                source: None,
            }
        } else {
            Self {
                layer: depth,
                source: Some(Box::new(Self::with_depth(depth - 1))),
            }
        }
    }
}

impl fmt::Display for ChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "layer {}", self.layer)
    }
}

impl Error for ChainError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}
