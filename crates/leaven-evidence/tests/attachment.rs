use leaven_evidence::{Attachment, AttachmentKind};
use leaven_kernel::TraceRef;

#[test]
fn attachment_serializes_typed_evidence_variants() {
    let trace = TraceRef {
        store: "trace-store".to_owned(),
        key: "session/main".to_owned(),
    };
    let attachments = vec![
        Attachment {
            name: "session/main".to_owned(),
            kind: AttachmentKind::Transcript(trace.clone()),
            media_type: Some("text/markdown".to_owned()),
        },
        Attachment {
            name: "metrics".to_owned(),
            kind: AttachmentKind::Json(serde_json::json!({"score": 0.42})),
            media_type: Some("application/json".to_owned()),
        },
        Attachment {
            name: "notes".to_owned(),
            kind: AttachmentKind::Text("look here".to_owned()),
            media_type: Some("text/plain".to_owned()),
        },
        Attachment {
            name: "events".to_owned(),
            kind: AttachmentKind::File { ref_: trace },
            media_type: Some("application/jsonl".to_owned()),
        },
    ];

    let encoded = serde_json::to_string(&attachments).unwrap();
    assert!(encoded.contains("\"kind\":\"transcript\""));
    assert!(encoded.contains("\"kind\":\"json\""));
    assert!(encoded.contains("\"kind\":\"text\""));
    assert!(encoded.contains("\"kind\":\"file\""));

    let decoded: Vec<Attachment> = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, attachments);
}
