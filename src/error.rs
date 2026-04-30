pub(crate) type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct BotError(String);

pub(crate) fn user_error(message: impl Into<String>) -> Error {
    Box::new(BotError(message.into()))
}
