use std::collections::BTreeSet;
use std::fs;

use super::PublicSeamPackage;
use super::support::{evidence_is_only_known_fake_passes, looks_like_denial_test};
use crate::{ConformanceMatrix, ConformanceRow, MatrixRowStatus, PublicSeamError};

pub(super) fn audit_conformance_evidence(
    package: &PublicSeamPackage,
    matrix: &ConformanceMatrix,
) -> Result<(), PublicSeamError> {
    package.validate_matrix_references(matrix)?;
    let denominator = package.conformance_test_denominator()?;
    let mut mapped_cases = BTreeSet::new();
    for row in &matrix.rows {
        for case_id in &row.conformance_tests {
            mapped_cases.insert(case_id.as_str());
        }
    }
    for case in &denominator.cases {
        if !mapped_cases.contains(case.id.as_str()) {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!(
                    "conformance test `{}` is not mapped to a matrix row",
                    case.id
                ),
            });
        }
    }
    for row in &matrix.rows {
        validate_blocked_status(row)?;
        if row.status != MatrixRowStatus::Proven {
            validate_non_proven_evidence(row)?;
            continue;
        }
        if row.implementation_evidence.is_empty() {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!("row `{}` is proven without implementation evidence", row.id),
            });
        }
        if row.review_evidence.is_empty() {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!("row `{}` is proven without review evidence", row.id),
            });
        }
        ensure_row_has_closeout_review(package, row)?;
        if row.fake_pass_rejected.trim().is_empty() {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!("row `{}` does not name the fake pass it rejects", row.id),
            });
        }
        if evidence_is_only_known_fake_passes(&row.implementation_evidence) {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!(
                    "row `{}` implementation evidence is only schema/example/topology/matrix proof",
                    row.id
                ),
            });
        }
        if row.positive_test_evidence.is_empty() {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!("row `{}` lacks positive test evidence", row.id),
            });
        }
        for reference in &row.positive_test_evidence {
            ensure_test_reference(package, &row.id, reference)?;
        }
        if row.minimum_closeout_level.requires_denial_evidence() {
            if row.negative_test_evidence.is_empty() {
                return Err(PublicSeamError::InvalidMatrix {
                    message: format!("row `{}` lacks negative test evidence", row.id),
                });
            }
            for reference in &row.negative_test_evidence {
                ensure_test_reference(package, &row.id, reference)?;
            }
            for reference in &row.negative_test_evidence {
                ensure_denial_test_reference(&row.id, reference)?;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_matrix_references(
    package: &PublicSeamPackage,
    matrix: &ConformanceMatrix,
) -> Result<(), PublicSeamError> {
    for row in &matrix.rows {
        for reference in &row.spec_refs {
            ensure_matrix_reference(package, &row.id, reference)?;
        }
        for reference in row
            .implementation_evidence
            .iter()
            .chain(row.partial_contract_implementation_evidence.iter())
            .chain(row.review_evidence.iter())
        {
            ensure_matrix_reference(package, &row.id, reference)?;
        }
        for reference in row
            .positive_test_evidence
            .iter()
            .chain(row.negative_test_evidence.iter())
            .chain(row.partial_contract_test_evidence.iter())
        {
            ensure_test_reference(package, &row.id, reference)?;
        }
    }
    Ok(())
}

pub(super) fn ensure_matrix_reference(
    package: &PublicSeamPackage,
    row_id: &str,
    reference: &str,
) -> Result<(), PublicSeamError> {
    let path_part = reference
        .split_once("::")
        .map_or(reference, |(path, _)| path);
    let path = package.repo_root.join(path_part);
    if path.exists() {
        Ok(())
    } else {
        Err(PublicSeamError::InvalidMatrix {
            message: format!("row `{row_id}` references missing `{reference}`"),
        })
    }
}

pub(super) fn ensure_test_reference(
    package: &PublicSeamPackage,
    row_id: &str,
    reference: &str,
) -> Result<(), PublicSeamError> {
    let (path_part, symbol) =
        reference
            .split_once("::")
            .ok_or_else(|| PublicSeamError::InvalidMatrix {
                message: format!("row `{row_id}` test evidence `{reference}` has no symbol"),
            })?;
    let path = package.repo_root.join(path_part);
    let source = fs::read_to_string(&path).map_err(|source| PublicSeamError::InvalidMatrix {
        message: format!(
            "row `{row_id}` test evidence `{reference}` could not be read at `{}`: {source}",
            path.display()
        ),
    })?;
    let symbol = symbol.rsplit("::").next().unwrap_or(symbol);
    if source.contains(&format!("fn {symbol}(")) {
        Ok(())
    } else {
        Err(PublicSeamError::InvalidMatrix {
            message: format!("row `{row_id}` test evidence `{reference}` is missing"),
        })
    }
}

fn validate_blocked_status(row: &ConformanceRow) -> Result<(), PublicSeamError> {
    if row.status != MatrixRowStatus::Blocked && !row.blocked_on.is_empty() {
        return Err(PublicSeamError::InvalidMatrix {
            message: format!(
                "row `{}` carries blocked_on prerequisites but is not blocked",
                row.id
            ),
        });
    }
    if row.status == MatrixRowStatus::Blocked
        && (row.blocked_on.is_empty()
            || row
                .blocked_on
                .iter()
                .any(|prerequisite| prerequisite.trim().is_empty()))
    {
        return Err(PublicSeamError::InvalidMatrix {
            message: format!(
                "row `{}` is blocked without concrete blocked_on prerequisites",
                row.id
            ),
        });
    }
    Ok(())
}

fn validate_non_proven_evidence(row: &ConformanceRow) -> Result<(), PublicSeamError> {
    if !row.positive_test_evidence.is_empty()
        || !row.negative_test_evidence.is_empty()
        || !row.implementation_evidence.is_empty()
    {
        return Err(PublicSeamError::InvalidMatrix {
            message: format!(
                "row `{}` is not proven but uses closeout evidence fields instead of partial_contract evidence",
                row.id
            ),
        });
    }
    Ok(())
}

fn ensure_row_has_closeout_review(
    package: &PublicSeamPackage,
    row: &ConformanceRow,
) -> Result<(), PublicSeamError> {
    for reference in &row.review_evidence {
        let path_part = reference
            .split_once("::")
            .map_or(reference.as_str(), |(path, _)| path);
        let path = package.repo_root.join(path_part);
        let source =
            fs::read_to_string(&path).map_err(|source| PublicSeamError::InvalidMatrix {
                message: format!(
                    "row `{}` review evidence `{reference}` could not be read at `{}`: {source}",
                    row.id,
                    path.display()
                ),
            })?;
        if review_text_signs_off_row(&source, &row.id) {
            return Ok(());
        }
    }
    Err(PublicSeamError::InvalidMatrix {
        message: format!(
            "row `{}` is proven without row-specific adversarial sign-off review evidence",
            row.id
        ),
    })
}

fn review_text_signs_off_row(source: &str, row_id: &str) -> bool {
    if !source.contains(row_id) {
        return false;
    }
    for line in source.lines() {
        if review_text_block_signs_off_row(line, row_id) {
            return true;
        }
    }
    if signed_off_rows_section_contains_row(source, row_id) {
        return true;
    }
    if explicit_signoff_section_contains_row(source, row_id) {
        return true;
    }
    if review_text_has_single_row_scope(source, row_id)
        && !review_text_has_row_specific_closeout_rejection(source, row_id)
    {
        return source.lines().any(|line| {
            review_text_has_document_verdict(line) && {
                let normalized = line.to_ascii_lowercase();
                !review_text_block_rejects_closeout(&normalized)
            }
        });
    }
    false
}

fn review_text_has_row_specific_closeout_rejection(source: &str, row_id: &str) -> bool {
    source.lines().any(|line| {
        line.contains(row_id) && review_text_block_rejects_closeout(&line.to_ascii_lowercase())
    })
}

fn review_text_block_signs_off_row(block: &str, row_id: &str) -> bool {
    if !block.contains(row_id) {
        return false;
    }
    let normalized = block.to_ascii_lowercase();
    let row = format!("`{row_id}`").to_ascii_lowercase();
    let has_signoff = normalized.contains(&format!("{row}: signed off"))
        || normalized.contains(&format!("{row}: sign off"))
        || normalized.contains(&format!("{row}: sign-off"))
        || normalized.contains(&format!("{row}: signoff"))
        || normalized.contains(&format!("{row}: may be promoted"))
        || normalized.contains(&format!("{row}: can be promoted"))
        || normalized.contains(&format!("{row}: can move to proven"))
        || normalized.contains(&format!("{row}: can move to `proven`"))
        || normalized.contains(&format!("{row}: can be marked proven"))
        || normalized.contains(&format!("{row} may be marked proven"))
        || normalized.contains(&format!("{row} may move to proven"))
        || normalized.contains(&format!("{row}: proven"))
        || normalized.contains(&format!("sign off on promoting {row}"))
        || normalized.contains(&format!("signed off for {row}"))
        || normalized.contains(&format!("sign-off for {row}"))
        || normalized.contains(&format!("promote {row} to `proven`"))
        || normalized.contains(&format!("promote {row} to proven"))
        || normalized.contains(&format!("no blocking findings remain for {row}"))
        || normalized.contains(&format!("{row} has no blocking finding"));
    if !has_signoff {
        return false;
    }
    !review_text_block_rejects_closeout(&normalized)
}

fn review_text_block_rejects_closeout(normalized: &str) -> bool {
    [
        "partial evidence only",
        "partial pending-row evidence",
        "pending-row evidence only",
        "row remains pending",
        "must remain pending",
        "remain pending",
        "remains pending",
        "remain `pending`",
        "remains `pending`",
        "not full row closeout",
        "not full",
        "not row closeout",
        "not proven",
        "do not mark",
        "not allowed",
        "not as sign-off",
        "no row was promoted",
        "pending after review",
        "still pending",
        "required before",
        "still required",
        "before the row can move",
        "does not sign off",
        "does not promote",
        "do not promote",
        "blocks row promotion",
        "block row promotion",
        "not promote",
        "no sign-off",
    ]
    .iter()
    .any(|phrase| normalized.contains(phrase))
}

fn signed_off_rows_section_contains_row(source: &str, row_id: &str) -> bool {
    let mut in_signed_off_rows = false;
    for line in source.lines() {
        let normalized = line.to_ascii_lowercase();
        if normalized.starts_with("## ") || normalized.starts_with("# ") {
            in_signed_off_rows = normalized.contains("signed off rows");
            continue;
        }
        if in_signed_off_rows && normalized.starts_with("## ") {
            in_signed_off_rows = false;
        }
        if in_signed_off_rows
            && line.contains(row_id)
            && !review_text_block_rejects_closeout(&normalized)
        {
            return true;
        }
    }
    false
}

fn explicit_signoff_section_contains_row(source: &str, row_id: &str) -> bool {
    let mut in_signoff_section = false;
    for line in source.lines() {
        let normalized = line.to_ascii_lowercase();
        let starts_section = normalized.contains("reviewer sign-off")
            || normalized.contains("sign-off:")
            || normalized.contains("sign off:")
            || normalized.contains("final verdict")
            || normalized.contains("final decision");
        if starts_section && !review_text_block_rejects_closeout(&normalized) {
            in_signoff_section = true;
            continue;
        }
        if in_signoff_section && normalized.starts_with("## ") {
            in_signoff_section = false;
        }
        if in_signoff_section
            && line.contains(row_id)
            && line.trim_start().starts_with("- `")
            && !review_text_block_rejects_closeout(&normalized)
        {
            return true;
        }
    }
    false
}

fn review_text_has_single_row_scope(source: &str, row_id: &str) -> bool {
    let first_lines = source.lines().take(20).collect::<Vec<_>>().join("\n");
    first_lines.contains(row_id)
}

fn review_text_has_document_verdict(source: &str) -> bool {
    let normalized = source.to_ascii_lowercase();
    normalized.contains("verdict: sign off")
        || normalized.contains("verdict: sign-off")
        || normalized.contains("may be promoted")
        || normalized.contains("can move to `proven`")
        || normalized.contains("can move to proven")
        || normalized.contains("final verdict")
        || normalized.contains("final decision")
}

fn ensure_denial_test_reference(row_id: &str, reference: &str) -> Result<(), PublicSeamError> {
    let (_, symbol) = reference
        .split_once("::")
        .ok_or_else(|| PublicSeamError::InvalidMatrix {
            message: format!("row `{row_id}` test evidence `{reference}` has no symbol"),
        })?;
    let symbol = symbol.rsplit("::").next().unwrap_or(symbol);
    if looks_like_denial_test(symbol) {
        Ok(())
    } else {
        Err(PublicSeamError::InvalidMatrix {
            message: format!(
                "row `{row_id}` negative test evidence `{reference}` does not look like denial evidence"
            ),
        })
    }
}
