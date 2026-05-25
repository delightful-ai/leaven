use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct BenchCase {
    pub(super) name: String,
    pub(super) file_count: usize,
    pub(super) file_bytes: usize,
}

impl BenchCase {
    pub(super) const fn total_bytes(&self) -> usize {
        self.file_count * self.file_bytes
    }
}

pub(super) fn default_cases() -> Vec<BenchCase> {
    vec![
        BenchCase {
            name: "small".to_owned(),
            file_count: 100,
            file_bytes: 1024,
        },
        BenchCase {
            name: "medium".to_owned(),
            file_count: 1000,
            file_bytes: 4096,
        },
        BenchCase {
            name: "large".to_owned(),
            file_count: 5000,
            file_bytes: 4096,
        },
    ]
}

pub(super) fn parse_case(raw: &str) -> std::result::Result<BenchCase, String> {
    let mut parts = raw.split(':');
    let name = parts
        .next()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "case name is required".to_owned())?;
    let file_count = parts
        .next()
        .ok_or_else(|| "file count is required".to_owned())?
        .parse::<usize>()
        .map_err(|source| format!("invalid file count: {source}"))?;
    let file_bytes = parts
        .next()
        .ok_or_else(|| "file bytes is required".to_owned())?
        .parse::<usize>()
        .map_err(|source| format!("invalid file bytes: {source}"))?;
    if parts.next().is_some() {
        return Err("case must be NAME:FILES:BYTES".to_owned());
    }
    if file_count == 0 || file_bytes == 0 {
        return Err("file count and bytes must be positive".to_owned());
    }
    Ok(BenchCase {
        name: name.to_owned(),
        file_count,
        file_bytes,
    })
}
