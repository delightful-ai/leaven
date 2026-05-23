use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::{ACTIVE_PACKAGE_RELATIVE, ConformanceTestKind, Manifest};
use crate::PublicSeamError;

pub(super) fn evidence_is_only_known_fake_passes(references: &[String]) -> bool {
    references.iter().all(|reference| {
        let path = reference
            .split_once("::")
            .map_or(reference.as_str(), |(path, _)| path);
        path.starts_with("docs/specs/public-seam-v1/schemas/")
            || path.starts_with("docs/specs/public-seam-v1/examples/")
            || path == "docs/specs/public-seam-v1/conformance-matrix.yaml"
            || path == "crates/leaven/tests/topology_contract.rs"
    })
}

pub(super) fn backtick_tokens(contents: &str) -> impl Iterator<Item = &str> {
    contents
        .split('`')
        .enumerate()
        .filter_map(|(index, token)| (index % 2 == 1).then_some(token))
}

pub(super) fn conformance_case_id(kind: ConformanceTestKind, text: &str) -> String {
    let prefix = match kind {
        ConformanceTestKind::Reject => "reject",
        ConformanceTestKind::Accept => "accept",
    };
    let mut words = Vec::new();
    for raw in text.split(|character: char| !character.is_ascii_alphanumeric()) {
        if raw.is_empty() {
            continue;
        }
        let word = raw.to_ascii_lowercase();
        if matches!(
            word.as_str(),
            "a" | "an" | "the" | "that" | "whose" | "with" | "without" | "and" | "or" | "of"
        ) {
            continue;
        }
        words.push(word);
    }
    format!("{prefix}_{}", words.join("_"))
}

pub(super) fn looks_like_denial_test(symbol: &str) -> bool {
    [
        "reject",
        "rejects",
        "refuse",
        "refuses",
        "deny",
        "denies",
        "denial",
        "invalid",
        "missing",
        "mismatch",
        "outside",
        "forbidden",
        "cannot",
        "fake",
        "negative",
        "only",
        "not_",
    ]
    .iter()
    .any(|term| symbol.contains(term))
}

pub(super) fn is_active_package_path(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("public-seam-v1")
        && path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some("specs")
        && path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some("docs")
}

pub(super) fn is_canonical_active_package(path: &Path) -> bool {
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    let Ok(active) = source_repo_root()
        .join(ACTIVE_PACKAGE_RELATIVE)
        .canonicalize()
    else {
        return false;
    };
    path == active
}

fn source_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("leaven-public-seam lives under workspace/crates")
        .to_path_buf()
}

pub(super) fn read_manifest(path: &Path) -> Result<Manifest, PublicSeamError> {
    read_json_value(path).and_then(|value| {
        serde_json::from_value(value).map_err(|error| PublicSeamError::InvalidManifest {
            message: error.to_string(),
        })
    })
}

pub(super) fn read_json(path: impl AsRef<Path>) -> Result<Value, PublicSeamError> {
    read_json_value(path.as_ref())
}

fn read_json_value(path: &Path) -> Result<Value, PublicSeamError> {
    let text = read_to_string(path)?;
    serde_json::from_str(&text).map_err(|source| PublicSeamError::Json {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn read_yaml<T>(path: &Path) -> Result<T, PublicSeamError>
where
    T: for<'de> Deserialize<'de>,
{
    let text = read_to_string(path)?;
    serde_yml::from_str(&text).map_err(|source| PublicSeamError::Yaml {
        path: path.to_path_buf(),
        source,
    })
}

fn read_to_string(path: &Path) -> Result<String, PublicSeamError> {
    fs::read_to_string(path).map_err(|source| PublicSeamError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn ensure_exists(path: &Path) -> Result<(), PublicSeamError> {
    if path.exists() {
        Ok(())
    } else {
        Err(PublicSeamError::MissingContractFile {
            path: path.to_path_buf(),
        })
    }
}
