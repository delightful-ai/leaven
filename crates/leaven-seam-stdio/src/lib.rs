//! Stdio adapter for the Leaven public seam runtime.

mod stdio;

pub use stdio::{SeamStdioError, StdioServeReport, serve_inherited_stdio, serve_reader_writer};
