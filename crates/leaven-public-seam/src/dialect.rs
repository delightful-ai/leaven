use serde_json::Value;

use crate::PublicSeamError;

/// Evaluator for the public seam's pinned replay mini-languages.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PinnedDialectEvaluator;

impl PinnedDialectEvaluator {
    /// Resolves an RFC 6901 JSON Pointer against a JSON document.
    pub fn resolve_json_pointer(
        &self,
        document: &Value,
        pointer: &str,
    ) -> Result<Value, PublicSeamError> {
        let tokens = parse_json_pointer(pointer)?;
        let mut current = document;
        for token in tokens {
            current = match current {
                Value::Object(object) => object.get(&token).ok_or_else(|| {
                    invalid_dialect(format!("JSON Pointer segment `{token}` was not present"))
                })?,
                Value::Array(array) => {
                    let index = token.parse::<usize>().map_err(|_| {
                        invalid_dialect(format!(
                            "JSON Pointer array index `{token}` is not numeric"
                        ))
                    })?;
                    array.get(index).ok_or_else(|| {
                        invalid_dialect(format!(
                            "JSON Pointer array index `{index}` was out of bounds"
                        ))
                    })?
                }
                _ => {
                    return Err(invalid_dialect(
                        "JSON Pointer cannot descend through a scalar value",
                    ));
                }
            };
        }
        Ok(current.clone())
    }

    /// Extracts values with the Leaven RFC 9535 `JSONPath` subset.
    pub fn extract_json_path(
        &self,
        document: &Value,
        path: &str,
    ) -> Result<Vec<Value>, PublicSeamError> {
        let segments = parse_json_path(path)?;
        let mut current = vec![document];
        for segment in segments {
            let mut next = Vec::new();
            for value in current {
                match &segment {
                    JsonPathSegment::Child(name) => {
                        if let Value::Object(object) = value {
                            if let Some(child) = object.get(name) {
                                next.push(child);
                            }
                        }
                    }
                    JsonPathSegment::Index(index) => {
                        if let Value::Array(array) = value {
                            if let Some(child) = array.get(*index) {
                                next.push(child);
                            }
                        }
                    }
                    JsonPathSegment::Wildcard => match value {
                        Value::Array(array) => next.extend(array.iter()),
                        Value::Object(object) => next.extend(object.values()),
                        _ => {}
                    },
                    JsonPathSegment::Slice { start, end } => {
                        if let Value::Array(array) = value {
                            let lower = start.unwrap_or(0).min(array.len());
                            let upper = end.unwrap_or(array.len()).min(array.len());
                            if lower <= upper {
                                next.extend(array[lower..upper].iter());
                            }
                        }
                    }
                }
            }
            current = next;
        }
        Ok(current.into_iter().cloned().collect())
    }

    /// Renders the strict Leaven Mustache dialect.
    pub fn render_template(
        &self,
        dialect: &str,
        template: &str,
        vars: &Value,
    ) -> Result<String, PublicSeamError> {
        if dialect != "leaven.mustache.strict.v1" {
            return Err(invalid_dialect(format!(
                "template dialect `{dialect}` is not leaven.mustache.strict.v1"
            )));
        }
        render_template_body(template, vars)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum JsonPathSegment {
    Child(String),
    Index(usize),
    Wildcard,
    Slice {
        start: Option<usize>,
        end: Option<usize>,
    },
}

fn parse_json_pointer(pointer: &str) -> Result<Vec<String>, PublicSeamError> {
    if pointer.is_empty() {
        return Ok(Vec::new());
    }
    if !pointer.starts_with('/') {
        return Err(invalid_dialect(
            "JSON Pointer must be empty or start with `/`",
        ));
    }
    pointer
        .split('/')
        .skip(1)
        .map(|segment| {
            let mut decoded = String::new();
            let mut chars = segment.chars();
            while let Some(ch) = chars.next() {
                if ch == '~' {
                    match chars.next() {
                        Some('0') => decoded.push('~'),
                        Some('1') => decoded.push('/'),
                        _ => {
                            return Err(invalid_dialect(
                                "JSON Pointer escape must be `~0` or `~1`",
                            ));
                        }
                    }
                } else {
                    decoded.push(ch);
                }
            }
            Ok(decoded)
        })
        .collect()
}

fn parse_json_path(path: &str) -> Result<Vec<JsonPathSegment>, PublicSeamError> {
    if !path.starts_with('$') {
        return Err(invalid_dialect("JSONPath must start at `$`"));
    }
    if path.contains('?')
        || path.contains('@')
        || path.contains('(')
        || path.contains(')')
        || path.contains("..")
    {
        return Err(invalid_dialect(
            "Leaven JSONPath excludes filters, scripts, functions, and recursive descent",
        ));
    }
    let bytes = path.as_bytes();
    let mut index = 1;
    let mut segments = Vec::new();
    while index < bytes.len() {
        match bytes[index] {
            b'.' => {
                index += 1;
                let start = index;
                while index < bytes.len() && is_jsonpath_name_byte(bytes[index]) {
                    index += 1;
                }
                if start == index {
                    return Err(invalid_dialect("JSONPath child name after `.` is empty"));
                }
                segments.push(JsonPathSegment::Child(path[start..index].to_owned()));
            }
            b'[' => {
                let close = path[index + 1..]
                    .find(']')
                    .map(|offset| index + 1 + offset)
                    .ok_or_else(|| invalid_dialect("JSONPath bracket segment is unterminated"))?;
                let inner = &path[index + 1..close];
                segments.push(parse_bracket_segment(inner)?);
                index = close + 1;
            }
            _ => {
                return Err(invalid_dialect(
                    "JSONPath segment must be a child or bracket segment",
                ));
            }
        }
    }
    Ok(segments)
}

fn parse_bracket_segment(inner: &str) -> Result<JsonPathSegment, PublicSeamError> {
    if inner == "*" {
        return Ok(JsonPathSegment::Wildcard);
    }
    if let Some(name) = quoted_child_name(inner) {
        return Ok(JsonPathSegment::Child(name.to_owned()));
    }
    if let Some((start, end)) = inner.split_once(':') {
        return Ok(JsonPathSegment::Slice {
            start: parse_optional_usize(start)?,
            end: parse_optional_usize(end)?,
        });
    }
    let index = inner
        .parse::<usize>()
        .map_err(|_| invalid_dialect(format!("unsupported JSONPath bracket segment `{inner}`")))?;
    Ok(JsonPathSegment::Index(index))
}

fn quoted_child_name(inner: &str) -> Option<&str> {
    inner
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .or_else(|| {
            inner
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
        })
}

fn parse_optional_usize(raw: &str) -> Result<Option<usize>, PublicSeamError> {
    if raw.is_empty() {
        Ok(None)
    } else {
        raw.parse::<usize>()
            .map(Some)
            .map_err(|_| invalid_dialect(format!("JSONPath slice bound `{raw}` is not numeric")))
    }
}

fn is_jsonpath_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn render_template_body(template: &str, vars: &Value) -> Result<String, PublicSeamError> {
    let mut output = String::new();
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        output.push_str(&rest[..open]);
        let after_open = &rest[open + 2..];
        if after_open.starts_with('{') {
            return Err(invalid_dialect(
                "triple mustache is not in the strict dialect",
            ));
        }
        let close = after_open
            .find("}}")
            .ok_or_else(|| invalid_dialect("unterminated mustache tag"))?;
        let tag = after_open[..close].trim();
        rest = &after_open[close + 2..];
        match tag.chars().next() {
            Some('#' | '^') => {
                let inverted = tag.starts_with('^');
                let name = tag[1..].trim();
                validate_mustache_name(name)?;
                let close_tag = format!("{{{{/{name}}}}}");
                let end = rest
                    .find(&close_tag)
                    .ok_or_else(|| invalid_dialect(format!("missing closing section `{name}`")))?;
                let body = &rest[..end];
                rest = &rest[end + close_tag.len()..];
                let value = lookup_dotted(vars, name);
                let truthy = value.is_some_and(is_truthy);
                if inverted && !truthy {
                    output.push_str(&render_template_body(body, vars)?);
                } else if !inverted {
                    render_section(body, value, vars, &mut output)?;
                }
            }
            Some('/' | '>' | '&' | '!' | '=') => {
                return Err(invalid_dialect(format!(
                    "mustache tag `{tag}` is not in the strict dialect"
                )));
            }
            Some(_) => {
                validate_mustache_name(tag)?;
                if let Some(value) = lookup_dotted(vars, tag) {
                    output.push_str(&scalar_to_template_text(value)?);
                }
            }
            None => return Err(invalid_dialect("empty mustache tag")),
        }
    }
    output.push_str(rest);
    Ok(output)
}

fn render_section(
    body: &str,
    value: Option<&Value>,
    root: &Value,
    output: &mut String,
) -> Result<(), PublicSeamError> {
    match value {
        Some(Value::Array(values)) => {
            for item in values {
                output.push_str(&render_template_body(body, item)?);
            }
        }
        Some(Value::Object(_)) => output.push_str(&render_template_body(body, value.unwrap())?),
        Some(value) if is_truthy(value) => output.push_str(&render_template_body(body, root)?),
        _ => {}
    }
    Ok(())
}

fn lookup_dotted<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in name.split('.') {
        current = current.as_object()?.get(part)?;
    }
    Some(current)
}

fn validate_mustache_name(name: &str) -> Result<(), PublicSeamError> {
    if name.is_empty() || name.contains('|') || name.contains(char::is_whitespace) {
        return Err(invalid_dialect(format!(
            "invalid strict mustache name `{name}`"
        )));
    }
    for part in name.split('.') {
        let mut chars = part.chars();
        let Some(first) = chars.next() else {
            return Err(invalid_dialect(format!(
                "invalid strict mustache name `{name}`"
            )));
        };
        if !(first.is_ascii_alphabetic() || first == '_') {
            return Err(invalid_dialect(format!(
                "invalid strict mustache name `{name}`"
            )));
        }
        if !chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':')) {
            return Err(invalid_dialect(format!(
                "invalid strict mustache name `{name}`"
            )));
        }
    }
    Ok(())
}

fn scalar_to_template_text(value: &Value) -> Result<String, PublicSeamError> {
    match value {
        Value::Null => Ok(String::new()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Number(value) => Ok(value.to_string()),
        Value::String(value) => Ok(value.clone()),
        Value::Array(_) | Value::Object(_) => Err(invalid_dialect(
            "strict mustache variables must resolve to scalar values",
        )),
    }
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Array(values) => !values.is_empty(),
        Value::String(value) => !value.is_empty(),
        Value::Number(_) | Value::Object(_) => true,
    }
}

fn invalid_dialect(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidDialect {
        message: message.into(),
    }
}
