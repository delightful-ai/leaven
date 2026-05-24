use serde_json::{Map, Value};

pub(super) fn replayability_summary(
    values: &Map<String, Value>,
    receipts: &[Value],
) -> &'static str {
    let mut rank = receipts
        .iter()
        .filter_map(|receipt| {
            receipt
                .as_object()
                .and_then(|object| object.get("kind"))
                .and_then(Value::as_str)
        })
        .filter(|kind| *kind != "query")
        .map(|_| 1)
        .max()
        .unwrap_or(0);
    for value in values.values() {
        rank = rank.max(
            value
                .as_object()
                .and_then(|object| object.get("replayability"))
                .and_then(Value::as_str)
                .map(replayability_rank)
                .unwrap_or(0),
        );
    }
    match rank {
        0 => "pure_read",
        1 => "fully_managed",
        2 => "boundary_managed",
        3 => "has_declared_external_effects",
        _ => "has_untracked_external_effects",
    }
}

fn replayability_rank(replayability: &str) -> usize {
    match replayability {
        "pure_read" => 0,
        "fully_managed" => 1,
        "boundary_managed" => 2,
        "has_declared_external_effects" => 3,
        "has_untracked_external_effects" => 4,
        _ => 5,
    }
}
