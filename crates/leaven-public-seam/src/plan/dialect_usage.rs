use serde_json::Value;

use crate::{PinnedDialectEvaluator, PublicSeamError};

use super::parse::invalid_plan;

#[derive(Default)]
pub(super) struct DialectUsage {
    pub(super) pointers: usize,
    pub(super) jsonpaths: usize,
    pub(super) templates: usize,
    evaluator: PinnedDialectEvaluator,
}

impl DialectUsage {
    pub(super) fn inspect_value(&mut self, value: &Value) -> Result<(), PublicSeamError> {
        match value {
            Value::Object(object) => self.inspect_object(object),
            Value::Array(values) => {
                for value in values {
                    self.inspect_value(value)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn inspect_object(
        &mut self,
        object: &serde_json::Map<String, Value>,
    ) -> Result<(), PublicSeamError> {
        if let Some(pointer) = object.get("field").and_then(Value::as_str) {
            self.validate_pointer(pointer)?;
        }
        if object.get("kind").and_then(Value::as_str) == Some("stratified") {
            if let Some(pointer) = object.get("by").and_then(Value::as_str) {
                self.validate_pointer(pointer)?;
            }
        }
        if let Some(fields) = object.get("fields").and_then(Value::as_array) {
            for pointer in fields.iter().filter_map(Value::as_str) {
                self.validate_pointer(pointer)?;
            }
        }
        if object.get("kind").and_then(Value::as_str) == Some("extract") {
            if let Some(path) = object.get("path").and_then(Value::as_str) {
                self.validate_jsonpath(path)?;
            }
        }
        if object.get("kind").and_then(Value::as_str) == Some("template") {
            let dialect = object
                .get("dialect")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_plan("template expression must carry a dialect"))?;
            let template = object
                .get("template")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_plan("template expression must carry a template"))?;
            self.evaluator
                .render_template(dialect, template, &serde_json::json!({}))?;
            self.templates += 1;
        }
        for (key, value) in object {
            if object.get("kind").and_then(Value::as_str) == Some("events") && key == "filter" {
                continue;
            }
            if object.get("kind").and_then(Value::as_str) == Some("schema_valid") && key == "value"
            {
                self.inspect_value(value)?;
                continue;
            }
            if is_arbitrary_json_slot(key) {
                continue;
            }
            self.inspect_value(value)?;
        }
        Ok(())
    }

    fn validate_pointer(&mut self, pointer: &str) -> Result<(), PublicSeamError> {
        match self
            .evaluator
            .resolve_json_pointer(&serde_json::json!({}), pointer)
        {
            Ok(_) => {}
            Err(PublicSeamError::InvalidDialect { message })
                if message.contains("was not present")
                    || message.contains("out of bounds")
                    || message.contains("cannot descend") => {}
            Err(error) => return Err(error),
        }
        self.pointers += 1;
        Ok(())
    }

    fn validate_jsonpath(&mut self, path: &str) -> Result<(), PublicSeamError> {
        self.evaluator
            .extract_json_path(&serde_json::json!({}), path)?;
        self.jsonpaths += 1;
        Ok(())
    }
}

fn is_arbitrary_json_slot(key: &str) -> bool {
    matches!(
        key,
        "value"
            | "values"
            | "payload"
            | "scope"
            | "selector"
            | "provider_hints"
            | "schema"
            | "input_schema"
            | "metadata"
            | "rubric"
            | "causal"
            | "target"
            | "preference"
            | "ranking"
    )
}
