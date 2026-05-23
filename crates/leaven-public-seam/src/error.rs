use std::path::PathBuf;

/// Errors raised while loading or validating the locked public seam package.
#[derive(Debug, thiserror::Error)]
pub enum PublicSeamError {
    /// The caller tried to load a package other than the active V1 package.
    #[error("inactive public seam package `{path}`; V1 must load docs/specs/public-seam-v1")]
    InactivePackage {
        /// Path that was refused.
        path: PathBuf,
    },

    /// A manifest-listed contract file is missing.
    #[error("missing public seam contract file `{path}`")]
    MissingContractFile {
        /// Missing file path.
        path: PathBuf,
    },

    /// A file could not be read.
    #[error("failed to read `{path}`")]
    Io {
        /// Path being read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// JSON could not be parsed.
    #[error("invalid JSON in `{path}`")]
    Json {
        /// Path being parsed.
        path: PathBuf,
        /// Underlying parse error.
        #[source]
        source: serde_json::Error,
    },

    /// YAML could not be parsed.
    #[error("invalid YAML in `{path}`")]
    Yaml {
        /// Path being parsed.
        path: PathBuf,
        /// Underlying parse error.
        #[source]
        source: serde_yml::Error,
    },

    /// A manifest override did not match the locked manifest shape.
    #[error("invalid public seam manifest: {message}")]
    InvalidManifest {
        /// Human-readable reason.
        message: String,
    },

    /// A JSON Schema document is not valid Draft 2020-12.
    #[error("invalid public seam schema `{name}`: {message}")]
    InvalidSchema {
        /// Schema name.
        name: String,
        /// Validation or compilation error.
        message: String,
    },

    /// A JSON Schema instance failed validation.
    #[error("example `{example}` failed schema `{schema}` at `{pointer}`: {message}")]
    ExampleValidation {
        /// Example file path.
        example: PathBuf,
        /// Schema name.
        schema: String,
        /// JSON pointer within the example.
        pointer: String,
        /// Validation error.
        message: String,
    },

    /// A conformance matrix invariant failed.
    #[error("invalid public seam conformance matrix: {message}")]
    InvalidMatrix {
        /// Human-readable reason.
        message: String,
    },

    /// The manifest no longer encodes the locked V1 scope.
    #[error("invalid public seam V1 scope: {message}")]
    InvalidScope {
        /// Human-readable reason.
        message: String,
    },

    /// A schema-valid plan violates a public-seam semantic constraint.
    #[error("invalid public seam plan: {message}")]
    InvalidPlan {
        /// Human-readable reason.
        message: String,
    },

    /// A schema-valid plan result violates a public-seam semantic constraint.
    #[error("invalid public seam plan result: {message}")]
    InvalidPlanResult {
        /// Human-readable reason.
        message: String,
    },

    /// A schema-valid evidence envelope violates a public-seam semantic constraint.
    #[error("invalid public seam evidence envelope: {message}")]
    InvalidEvidence {
        /// Human-readable reason.
        message: String,
    },

    /// A schema-valid evaluation job violates a public-seam semantic constraint.
    #[error("invalid public seam evaluation job: {message}")]
    InvalidEvaluationJob {
        /// Human-readable reason.
        message: String,
    },

    /// A schema-valid output record violates a public-seam semantic constraint.
    #[error("invalid public seam output record: {message}")]
    InvalidOutputRecord {
        /// Human-readable reason.
        message: String,
    },

    /// A pinned public-seam mini-language rejected unsupported syntax.
    #[error("invalid public seam pinned dialect: {message}")]
    InvalidDialect {
        /// Human-readable reason.
        message: String,
    },

    /// A schema-valid watch marker or replacement plan violates the deferred V1 watch contract.
    #[error("invalid public seam deferred watch replacement: {message}")]
    InvalidWatch {
        /// Human-readable reason.
        message: String,
    },

    /// RFC 8785/JCS schema fingerprinting failed.
    #[error("schema fingerprinting failed: {message}")]
    Fingerprint {
        /// Human-readable reason.
        message: String,
    },
}
