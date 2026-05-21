//! Parquet adapters for lowered evaluation source rows.

mod reader;

pub use reader::{ParquetRow, ParquetSourceRowError, ParquetSourceRows, read_parquet_source_rows};
