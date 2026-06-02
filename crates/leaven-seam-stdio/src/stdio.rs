use std::io::{self, BufRead, Write};

use leaven_seam_runtime::{JsonRpcErrorCode, JsonRpcResponse, SeamRuntime, SeamService};
use serde_json::Value;

/// Summary of a stdio serve loop.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StdioServeReport {
    /// Number of non-empty request lines handled.
    pub requests: usize,
}

/// Serves the Leaven public seam over this process's inherited stdin/stdout.
pub fn serve_inherited_stdio<S>(
    runtime: &SeamRuntime<S>,
) -> Result<StdioServeReport, SeamStdioError>
where
    S: SeamService,
{
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve_reader_writer(runtime, stdin.lock(), stdout.lock())
}

/// Serves the Leaven public seam over a line-delimited reader/writer pair.
pub fn serve_reader_writer<R, W, S>(
    runtime: &SeamRuntime<S>,
    reader: R,
    mut writer: W,
) -> Result<StdioServeReport, SeamStdioError>
where
    R: BufRead,
    W: Write,
    S: SeamService,
{
    let mut report = StdioServeReport::default();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Value>(&line) {
            Ok(value) => runtime.handle_value(&value),
            Err(error) => JsonRpcResponse::error(
                Value::Null,
                JsonRpcErrorCode::ParseError,
                format!("failed to parse JSON-RPC line: {error}"),
            ),
        };

        serde_json::to_writer(&mut writer, response.value())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        report.requests += 1;
    }
    Ok(report)
}

/// Stdio adapter error.
#[derive(Debug, thiserror::Error)]
pub enum SeamStdioError {
    /// Reading or writing stdio failed.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Serializing a response failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
