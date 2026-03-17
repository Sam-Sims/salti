#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CommandError {
    pub(super) message: String,
}

impl CommandError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
