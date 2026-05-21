use std::fs::File;
use std::sync::Arc;

use arrow_array::builder::{ListBuilder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use leaven_eval::SourceRow;
use leaven_eval_parquet::{ParquetSourceRowError, read_parquet_source_rows};
use leaven_kernel::{CaseId, Fingerprint};
use parquet::arrow::ArrowWriter;

#[test]
fn reads_parquet_rows_into_ordered_source_row_manifest() {
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("sealqa-mini.parquet");
    write_sealqa_like_parquet(&path);

    let rows = read_parquet_source_rows(&path, |row| {
        Ok(SourceRow::targeted(
            row.required_string("canary")?,
            SealQaQuestion {
                question: row.required_string("question")?,
                topic: row.optional_string("topic")?,
                urls: row.optional_string_list("urls")?,
            },
            row.required_string("answer")?,
        ))
    })
    .unwrap();

    assert_eq!(rows.row_count(), 2);
    assert_ne!(rows.file_fingerprint(), Fingerprint::from_bytes([0; 32]));
    let manifest = rows.into_manifest();
    assert_eq!(
        manifest
            .rows()
            .iter()
            .map(leaven_eval::SourceRow::source_id)
            .collect::<Vec<_>>(),
        vec!["sealqa:row-0", "sealqa:row-1"]
    );

    let dataset = manifest.into_dataset().unwrap();
    let first = dataset.cases().get(&CaseId::from_index(0)).unwrap();
    assert_eq!(first.input.question, "first question");
    assert_eq!(first.input.topic.as_deref(), Some("Music"));
    assert_eq!(first.input.urls, vec!["https://example.test/0"]);
    assert_eq!(first.target.as_deref(), Some("first answer"));
}

#[test]
fn refuses_missing_required_string_columns() {
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("sealqa-mini.parquet");
    write_sealqa_like_parquet(&path);

    let error = read_parquet_source_rows(&path, |row| {
        Ok(SourceRow::targeted(
            row.required_string("missing")?,
            row.required_string("question")?,
            row.required_string("answer")?,
        ))
    })
    .unwrap_err();

    assert_eq!(
        error,
        ParquetSourceRowError::MissingColumn {
            column: "missing".to_owned()
        }
    );
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SealQaQuestion {
    question: String,
    topic: Option<String>,
    urls: Vec<String>,
}

fn write_sealqa_like_parquet(path: &std::path::Path) {
    let schema = Arc::new(Schema::new(vec![
        Field::new("canary", DataType::Utf8, false),
        Field::new("question", DataType::Utf8, false),
        Field::new("answer", DataType::Utf8, false),
        Field::new("topic", DataType::Utf8, true),
        Field::new(
            "urls",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
    ]));
    let urls = {
        let values = StringBuilder::new();
        let mut builder = ListBuilder::new(values);
        builder.values().append_value("https://example.test/0");
        builder.append(true);
        builder.values().append_value("https://example.test/1");
        builder.append(true);
        Arc::new(builder.finish()) as ArrayRef
    };
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(vec!["sealqa:row-0", "sealqa:row-1"])) as ArrayRef,
            Arc::new(StringArray::from(vec!["first question", "second question"])) as ArrayRef,
            Arc::new(StringArray::from(vec!["first answer", "second answer"])) as ArrayRef,
            Arc::new(StringArray::from(vec![Some("Music"), None])) as ArrayRef,
            urls,
        ],
    )
    .unwrap();

    let file = File::create(path).unwrap();
    let mut writer = ArrowWriter::try_new(file, schema, None).unwrap();
    writer.write(&batch).unwrap();
    writer.close().unwrap();
}
