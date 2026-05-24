use std::collections::BTreeSet;

use serde_json::Value;

use crate::PublicSeamError;

/// Schema-valid public-seam evidence envelope with visibility and data-class facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceEnvelopeDocument {
    target_derived: bool,
    data_classes: Vec<String>,
    public_data_classes: Vec<String>,
    private_data_classes: Option<Vec<String>>,
    trace_data_classes: Vec<String>,
    read_receipts: Vec<String>,
    effect_receipts: Vec<String>,
    write_receipts: Vec<String>,
}

impl EvidenceEnvelopeDocument {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_evidence("evidence envelope must be an object"))?;
        let target_derived = object
            .get("target_derived")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid_evidence("target_derived must be a boolean"))?;
        let data_classes = optional_string_vec(object.get("data_classes"), "data_classes")?;
        let public = object
            .get("public")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_evidence("public projection must be an object"))?;
        let public_data_classes =
            required_string_vec(public.get("data_classes"), "public.data_classes")?;
        let private_data_classes = object
            .get("private")
            .map(|private| {
                let private = private
                    .as_object()
                    .ok_or_else(|| invalid_evidence("private projection must be an object"))?;
                required_string_vec(private.get("data_classes"), "private.data_classes")
            })
            .transpose()?;
        let private_payload_data_classes = object
            .get("private")
            .and_then(Value::as_object)
            .and_then(|private| private.get("payload_ref"))
            .map(|payload_ref| collect_blob_ref_data_classes(payload_ref, "private.payload_ref"))
            .transpose()?
            .unwrap_or_default();
        validate_private_payload_ref_classes(
            private_data_classes.as_deref(),
            &private_payload_data_classes,
        )?;
        let trace_data_classes = public
            .get("trace_refs")
            .map(|trace_refs| collect_trace_data_classes(trace_refs, "public.trace_refs"))
            .transpose()?
            .unwrap_or_default();
        let top_level_trace_data_classes = object
            .get("trace_refs")
            .map(|trace_refs| collect_trace_data_classes(trace_refs, "trace_refs"))
            .transpose()?
            .unwrap_or_default();
        let trace_data_classes = trace_data_classes
            .into_iter()
            .chain(top_level_trace_data_classes)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let source_receipts = object
            .get("source_receipts")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_evidence("source_receipts must be an object"))?;
        let read_receipts =
            required_string_vec(source_receipts.get("read"), "source_receipts.read")?;
        let effect_receipts =
            required_string_vec(source_receipts.get("effect"), "source_receipts.effect")?;
        let write_receipts =
            optional_string_vec(source_receipts.get("write"), "source_receipts.write")?;
        if target_derived {
            validate_target_derived_classes(
                &data_classes,
                &public_data_classes,
                private_data_classes.as_deref(),
                &trace_data_classes,
            )?;
            if read_receipts.is_empty() {
                return Err(invalid_evidence(
                    "target-derived evidence must carry read receipts",
                ));
            }
        } else if carries_case_target_class(
            &data_classes,
            &public_data_classes,
            private_data_classes.as_deref(),
            &trace_data_classes,
        ) {
            return Err(invalid_evidence(
                "evidence carrying case.target data classes must declare target_derived=true",
            ));
        }
        if read_receipts.is_empty() && effect_receipts.is_empty() && write_receipts.is_empty() {
            return Err(invalid_evidence(
                "evidence source_receipts must carry at least one receipt",
            ));
        }
        Ok(Self {
            target_derived,
            data_classes,
            public_data_classes,
            private_data_classes,
            trace_data_classes,
            read_receipts,
            effect_receipts,
            write_receipts,
        })
    }

    /// Whether the envelope is derived from case target material.
    pub const fn is_target_derived(&self) -> bool {
        self.target_derived
    }

    /// Top-level data classes carried by this envelope.
    pub fn data_classes(&self) -> &[String] {
        &self.data_classes
    }

    /// Public projection data classes.
    pub fn public_data_classes(&self) -> &[String] {
        &self.public_data_classes
    }

    /// Private projection data classes, when a private projection is present.
    pub fn private_data_classes(&self) -> Option<&[String]> {
        self.private_data_classes.as_deref()
    }

    /// Data classes carried by public trace references.
    pub fn trace_data_classes(&self) -> &[String] {
        &self.trace_data_classes
    }

    /// Read receipt references used as evidence sources.
    pub fn read_receipts(&self) -> &[String] {
        &self.read_receipts
    }

    /// Effect receipt references used as evidence sources.
    pub fn effect_receipts(&self) -> &[String] {
        &self.effect_receipts
    }

    /// Write receipt references used as evidence sources.
    pub fn write_receipts(&self) -> &[String] {
        &self.write_receipts
    }
}

fn validate_target_derived_classes(
    data_classes: &[String],
    public_data_classes: &[String],
    private_data_classes: Option<&[String]>,
    trace_data_classes: &[String],
) -> Result<(), PublicSeamError> {
    let top_level = data_classes.iter().collect::<BTreeSet<_>>();
    if !top_level
        .iter()
        .any(|data_class| data_class.starts_with("case.target"))
    {
        return Err(invalid_evidence(
            "target-derived evidence data_classes must include case.target",
        ));
    }
    for data_class in public_data_classes
        .iter()
        .chain(private_data_classes.into_iter().flatten())
        .chain(trace_data_classes)
    {
        if !top_level.contains(data_class) {
            return Err(invalid_evidence(format!(
                "target-derived evidence data_classes must cover projection data class `{data_class}`"
            )));
        }
    }
    Ok(())
}

fn validate_private_payload_ref_classes(
    private_data_classes: Option<&[String]>,
    private_payload_data_classes: &[String],
) -> Result<(), PublicSeamError> {
    let Some(private_data_classes) = private_data_classes else {
        return Ok(());
    };
    let private_data_classes = private_data_classes.iter().collect::<BTreeSet<_>>();
    for data_class in private_payload_data_classes {
        if !private_data_classes.contains(data_class) {
            return Err(invalid_evidence(format!(
                "private.data_classes must cover private payload_ref data class `{data_class}`"
            )));
        }
    }
    Ok(())
}

fn carries_case_target_class(
    data_classes: &[String],
    public_data_classes: &[String],
    private_data_classes: Option<&[String]>,
    trace_data_classes: &[String],
) -> bool {
    data_classes
        .iter()
        .chain(public_data_classes)
        .chain(private_data_classes.into_iter().flatten())
        .chain(trace_data_classes)
        .any(|data_class| data_class.starts_with("case.target"))
}

fn collect_blob_ref_data_classes(
    value: &Value,
    field: &str,
) -> Result<Vec<String>, PublicSeamError> {
    let blob = value
        .as_object()
        .ok_or_else(|| invalid_evidence(format!("{field} must be an object")))?;
    optional_string_vec(blob.get("data_classes"), "blob_ref.data_classes")
}

fn collect_trace_data_classes(value: &Value, field: &str) -> Result<Vec<String>, PublicSeamError> {
    let traces = value
        .as_array()
        .ok_or_else(|| invalid_evidence(format!("{field} must be an array")))?;
    let mut data_classes = BTreeSet::new();
    for trace in traces {
        let trace = trace
            .as_object()
            .ok_or_else(|| invalid_evidence(format!("{field} entries must be objects")))?;
        if let Some(trace_data_classes) = trace.get("data_classes") {
            data_classes.extend(optional_string_vec(
                Some(trace_data_classes),
                "trace.data_classes",
            )?);
        }
    }
    Ok(data_classes.into_iter().collect())
}

fn required_string_vec(value: Option<&Value>, field: &str) -> Result<Vec<String>, PublicSeamError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_evidence(format!("{field} must be an array")))?;
    string_vec(values, field)
}

fn optional_string_vec(value: Option<&Value>, field: &str) -> Result<Vec<String>, PublicSeamError> {
    match value {
        Some(value) => {
            let values = value
                .as_array()
                .ok_or_else(|| invalid_evidence(format!("{field} must be an array")))?;
            string_vec(values, field)
        }
        None => Ok(Vec::new()),
    }
}

fn string_vec(values: &[Value], field: &str) -> Result<Vec<String>, PublicSeamError> {
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid_evidence(format!("{field} entries must be strings")))
        })
        .collect()
}

fn invalid_evidence(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidEvidence {
        message: message.into(),
    }
}
