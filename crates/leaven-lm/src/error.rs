#[derive(Debug, thiserror::Error)]
pub enum LmError {
    #[error("lm failed")]
    Message,
}
