use serde_json::Value;

use crate::PublicSeamError;

use super::parse::{invalid_plan, required_object_string};

/// Closed evaluation shapes supported by `request_evaluation`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanEvaluationShape {
    Independent,
    Pairwise,
    Listwise,
}

impl PlanEvaluationShape {
    pub(super) fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "independent" => Ok(Self::Independent),
            "pairwise" => Ok(Self::Pairwise),
            "listwise" => Ok(Self::Listwise),
            other => Err(invalid_plan(format!(
                "unknown request_evaluation shape `{other}`"
            ))),
        }
    }

    /// Wire spelling for this shape.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Independent => "independent",
            Self::Pairwise => "pairwise",
            Self::Listwise => "listwise",
        }
    }
}

/// Typed evaluation-set expression facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanEvaluationSetExpr {
    Named {
        name: String,
    },
    Cases {
        case_ids: Vec<String>,
        requires_partition_resolution: bool,
    },
    Tagged {
        tag: String,
        requires_partition_resolution: bool,
    },
    Recent {
        limit: u64,
        requires_partition_resolution: bool,
    },
    Union {
        sets: Vec<Self>,
    },
    Intersect {
        sets: Vec<Self>,
    },
    Difference {
        base: Box<Self>,
        subtract: Box<Self>,
    },
    Sample {
        base: Box<Self>,
        n: u64,
        seed: i64,
    },
    Stratified {
        base: Box<Self>,
        by: String,
        per_bucket: u64,
        seed: i64,
    },
}

impl PlanEvaluationSetExpr {
    pub(super) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_plan("request_evaluation set must be an object"))?;
        match required_object_string(object, "kind")? {
            "named" => Ok(Self::Named {
                name: required_object_string(object, "name")?.to_owned(),
            }),
            "cases" => Ok(Self::Cases {
                case_ids: required_array(object, "cases")?
                    .iter()
                    .map(case_ref_id)
                    .collect::<Result<Vec<_>, _>>()?,
                requires_partition_resolution: required_bool(
                    object,
                    "requires_partition_resolution",
                )?,
            }),
            "tagged" => Ok(Self::Tagged {
                tag: required_object_string(object, "tag")?.to_owned(),
                requires_partition_resolution: required_bool(
                    object,
                    "requires_partition_resolution",
                )?,
            }),
            "recent" => Ok(Self::Recent {
                limit: required_u64(object, "limit")?,
                requires_partition_resolution: required_bool(
                    object,
                    "requires_partition_resolution",
                )?,
            }),
            "union" => Ok(Self::Union {
                sets: evaluation_set_array(object, "sets")?,
            }),
            "intersect" => Ok(Self::Intersect {
                sets: evaluation_set_array(object, "sets")?,
            }),
            "difference" => Ok(Self::Difference {
                base: Box::new(evaluation_set_field(object, "base")?),
                subtract: Box::new(evaluation_set_field(object, "subtract")?),
            }),
            "sample" => Ok(Self::Sample {
                base: Box::new(evaluation_set_field(object, "base")?),
                n: required_u64(object, "n")?,
                seed: required_i64(object, "seed")?,
            }),
            "stratified" => Ok(Self::Stratified {
                base: Box::new(evaluation_set_field(object, "base")?),
                by: required_object_string(object, "by")?.to_owned(),
                per_bucket: required_u64(object, "per_bucket")?,
                seed: required_i64(object, "seed")?,
            }),
            other => Err(invalid_plan(format!(
                "unknown evaluation set kind `{other}`"
            ))),
        }
    }

    /// Evaluation-set expression kind.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Named { .. } => "named",
            Self::Cases { .. } => "cases",
            Self::Tagged { .. } => "tagged",
            Self::Recent { .. } => "recent",
            Self::Union { .. } => "union",
            Self::Intersect { .. } => "intersect",
            Self::Difference { .. } => "difference",
            Self::Sample { .. } => "sample",
            Self::Stratified { .. } => "stratified",
        }
    }

    /// Named-set identifier when this expression is `kind: "named"`.
    pub fn named_set(&self) -> Option<&str> {
        match self {
            Self::Named { name } => Some(name),
            Self::Cases { .. }
            | Self::Tagged { .. }
            | Self::Recent { .. }
            | Self::Union { .. }
            | Self::Intersect { .. }
            | Self::Difference { .. }
            | Self::Sample { .. }
            | Self::Stratified { .. } => None,
        }
    }
}

fn case_ref_id(value: &Value) -> Result<String, PublicSeamError> {
    if let Some(case_id) = value.as_str() {
        return Ok(case_id.to_owned());
    }
    value
        .as_object()
        .and_then(|object| object.get("id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| invalid_plan("request_evaluation case ref must carry id"))
}

fn evaluation_set_array(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<Vec<PlanEvaluationSetExpr>, PublicSeamError> {
    required_array(object, field)?
        .iter()
        .map(PlanEvaluationSetExpr::from_schema_valid_value)
        .collect()
}

fn evaluation_set_field(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<PlanEvaluationSetExpr, PublicSeamError> {
    PlanEvaluationSetExpr::from_schema_valid_value(
        object
            .get(field)
            .ok_or_else(|| invalid_plan(format!("evaluation set must carry {field}")))?,
    )
}

fn required_array<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<&'a [Value], PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid_plan(format!("expected evaluation set array `{field}`")))
}

fn required_bool(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<bool, PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid_plan(format!("expected evaluation set bool `{field}`")))
}

fn required_u64(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<u64, PublicSeamError> {
    object.get(field).and_then(Value::as_u64).ok_or_else(|| {
        invalid_plan(format!(
            "expected evaluation set unsigned integer `{field}`"
        ))
    })
}

fn required_i64(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<i64, PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid_plan(format!("expected evaluation set integer `{field}`")))
}
