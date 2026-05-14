use leaven_kernel::{AssessmentId, CandidateId};

use crate::{StageQuery, StageQueryKind};

pub fn parse_leaven_query_args(args: &[String]) -> Result<StageQuery, LeavenQueryCliError> {
    if let Some(flag) = args.iter().find(|arg| arg.starts_with("--")) {
        return Err(LeavenQueryCliError::UnknownFlag(flag.clone()));
    }
    match args.first().map(String::as_str) {
        None | Some("help") => Ok(StageQuery::Help),
        Some("list") => parse_list(&args[1..]),
        Some("candidate") => parse_candidate(&args[1..]),
        Some("assessment") => parse_assessment(&args[1..]),
        Some("evidence") => parse_evidence(&args[1..]),
        Some("lineage") => parse_lineage(&args[1..]),
        Some("diff") => parse_diff(&args[1..]),
        Some(command) => Err(LeavenQueryCliError::UnknownCommand(command.to_owned())),
    }
}

#[must_use]
pub fn leaven_query_help() -> String {
    let mut out = String::new();
    out.push_str("leaven_query help\n");
    for kind in StageQueryKind::all_v0_4() {
        out.push_str(kind.label());
        out.push('\n');
    }
    out.push_str("leaven_query list candidates\n");
    out.push_str("leaven_query candidate <candidate_id>\n");
    out.push_str("leaven_query assessment <assessment_id>\n");
    out.push_str("leaven_query evidence\n");
    out.push_str("leaven_query lineage <candidate_id> <depth>\n");
    out.push_str("leaven_query diff <left_candidate_id> <right_candidate_id>\n");
    out
}

#[derive(Debug, thiserror::Error)]
pub enum LeavenQueryCliError {
    #[error("unknown leaven_query command `{0}`")]
    UnknownCommand(String),
    #[error("unknown leaven_query flag `{0}`")]
    UnknownFlag(String),
    #[error("missing argument `{0}`")]
    MissingArgument(&'static str),
    #[error("invalid leaven_query arity for `{0}`")]
    InvalidArity(String),
    #[error("invalid {kind} id `{value}`")]
    InvalidId { kind: &'static str, value: String },
    #[error("invalid lineage depth `{0}`")]
    InvalidDepth(String),
}

fn parse_list(args: &[String]) -> Result<StageQuery, LeavenQueryCliError> {
    match args {
        [target] if target == "candidates" => Ok(StageQuery::ListCandidates),
        [] => Err(LeavenQueryCliError::MissingArgument("candidates")),
        [target] => Err(LeavenQueryCliError::UnknownCommand(format!(
            "list {target}"
        ))),
        _ => Err(LeavenQueryCliError::InvalidArity("list".to_owned())),
    }
}

fn parse_candidate(args: &[String]) -> Result<StageQuery, LeavenQueryCliError> {
    match args {
        [id] => Ok(StageQuery::Candidate {
            id: CandidateId::from_uuid(parse_uuid("candidate", id)?),
        }),
        [] => Err(LeavenQueryCliError::MissingArgument("candidate_id")),
        _ => Err(LeavenQueryCliError::InvalidArity("candidate".to_owned())),
    }
}

fn parse_assessment(args: &[String]) -> Result<StageQuery, LeavenQueryCliError> {
    match args {
        [id] => Ok(StageQuery::Assessment {
            id: AssessmentId::from_uuid(parse_uuid("assessment", id)?),
        }),
        [] => Err(LeavenQueryCliError::MissingArgument("assessment_id")),
        _ => Err(LeavenQueryCliError::InvalidArity("assessment".to_owned())),
    }
}

fn parse_evidence(args: &[String]) -> Result<StageQuery, LeavenQueryCliError> {
    match args {
        [] => Ok(StageQuery::Evidence),
        _ => Err(LeavenQueryCliError::InvalidArity("evidence".to_owned())),
    }
}

fn parse_lineage(args: &[String]) -> Result<StageQuery, LeavenQueryCliError> {
    match args {
        [candidate, depth] => Ok(StageQuery::Lineage {
            candidate: CandidateId::from_uuid(parse_uuid("candidate", candidate)?),
            depth: depth
                .parse()
                .map_err(|_| LeavenQueryCliError::InvalidDepth(depth.clone()))?,
        }),
        [] | [_] => Err(LeavenQueryCliError::MissingArgument("candidate_id depth")),
        _ => Err(LeavenQueryCliError::InvalidArity("lineage".to_owned())),
    }
}

fn parse_diff(args: &[String]) -> Result<StageQuery, LeavenQueryCliError> {
    match args {
        [left, right] => Ok(StageQuery::Diff {
            left: CandidateId::from_uuid(parse_uuid("left candidate", left)?),
            right: CandidateId::from_uuid(parse_uuid("right candidate", right)?),
        }),
        [] | [_] => Err(LeavenQueryCliError::MissingArgument(
            "left_candidate_id right_candidate_id",
        )),
        _ => Err(LeavenQueryCliError::InvalidArity("diff".to_owned())),
    }
}

fn parse_uuid(kind: &'static str, value: &str) -> Result<uuid::Uuid, LeavenQueryCliError> {
    value.parse().map_err(|_| LeavenQueryCliError::InvalidId {
        kind,
        value: value.to_owned(),
    })
}
