use std::time::Duration;

use leaven_evidence::{AgentTrajectoryEvidence, CommandEvidence, CommandRecord, OutputRecord};
use leaven_kernel::BlobRef;

#[test]
fn command_record_preserves_status_duration_and_inline_output() {
    let record = CommandRecord::new(
        "python harness.py",
        Some(0),
        OutputRecord::inline("ok"),
        OutputRecord::truncated("warning..."),
        Duration::from_millis(42),
    );

    assert_eq!(record.command(), "python harness.py");
    assert_eq!(record.exit_status(), Some(0));
    assert_eq!(record.duration(), Duration::from_millis(42));
    assert_eq!(record.stdout(), &OutputRecord::inline("ok"));
    assert_eq!(record.stderr(), &OutputRecord::truncated("warning..."));
}

#[test]
fn command_output_can_be_blob_backed() {
    let reference = BlobRef {
        store: "blob-store".to_owned(),
        key: "stdout/0".to_owned(),
    };

    assert_eq!(
        OutputRecord::blob(reference.clone()),
        OutputRecord::BlobRef(reference)
    );
}

#[test]
fn agent_trajectory_groups_transcript_and_commands() {
    let command = CommandRecord::new(
        "pytest",
        Some(1),
        OutputRecord::truncated("one failed"),
        OutputRecord::inline(""),
        Duration::from_secs(2),
    );
    let commands = CommandEvidence::new(vec![command.clone()]);
    let trajectory =
        AgentTrajectoryEvidence::new(OutputRecord::inline("agent transcript"), commands);

    assert_eq!(
        trajectory.transcript(),
        &OutputRecord::inline("agent transcript")
    );
    assert_eq!(trajectory.commands().records(), &[command]);
}
