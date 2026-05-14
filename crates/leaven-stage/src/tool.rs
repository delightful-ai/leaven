use crate::StageQuery;

pub fn parse_leaven_query_args(args: &[String]) -> Result<StageQuery, LeavenQueryCliError> {
    match args.first().map(String::as_str) {
        Some("help") | None => Ok(StageQuery::Help),
        Some(other) => Err(LeavenQueryCliError::UnknownCommand(other.to_owned())),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LeavenQueryCliError {
    #[error("unknown leaven_query command `{0}`")]
    UnknownCommand(String),
}
