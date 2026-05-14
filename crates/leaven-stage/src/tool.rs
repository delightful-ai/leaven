use leaven_kernel::{AssessmentId, CandidateId};

use crate::StageQuery;

pub fn parse_leaven_query_args(args: &[String]) -> Result<StageQuery, LeavenQueryCliError> {
    match args {
        [] => Ok(StageQuery::Help),
        [command] if command == "help" => Ok(StageQuery::Help),
        [command] if command == "list-candidates" => Ok(StageQuery::ListCandidates),
        [command, id] if command == "candidate" => Ok(StageQuery::Candidate {
            id: CandidateId::from_uuid(parse_uuid("candidate", id)?),
        }),
        [command, id] if command == "assessment" => Ok(StageQuery::Assessment {
            id: AssessmentId::from_uuid(parse_uuid("assessment", id)?),
        }),
        [command, candidate, depth] if command == "lineage" => Ok(StageQuery::Lineage {
            candidate: CandidateId::from_uuid(parse_uuid("candidate", candidate)?),
            depth: depth
                .parse()
                .map_err(|_| LeavenQueryCliError::InvalidDepth(depth.clone()))?,
        }),
        [command, left, right] if command == "diff" => Ok(StageQuery::Diff {
            left: CandidateId::from_uuid(parse_uuid("left candidate", left)?),
            right: CandidateId::from_uuid(parse_uuid("right candidate", right)?),
        }),
        [command] => Err(LeavenQueryCliError::UnknownCommand(command.to_owned())),
        [command, ..] => Err(LeavenQueryCliError::InvalidArity(command.to_owned())),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LeavenQueryCliError {
    #[error("unknown leaven_query command `{0}`")]
    UnknownCommand(String),
    #[error("invalid leaven_query arity for `{0}`")]
    InvalidArity(String),
    #[error("invalid {kind} id `{value}`")]
    InvalidId { kind: &'static str, value: String },
    #[error("invalid lineage depth `{0}`")]
    InvalidDepth(String),
}

fn parse_uuid(kind: &'static str, value: &str) -> Result<uuid::Uuid, LeavenQueryCliError> {
    value.parse().map_err(|_| LeavenQueryCliError::InvalidId {
        kind,
        value: value.to_owned(),
    })
}
