#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Could not authenticate with the chat server")]
    AuthenticateChatError,

    #[error("Did not receive a message from the chat server")]
    ReceivedNoMessageError,

    #[error("Could not communicate with chat server after {0} attempts")]
    FailedCommunicationError(u32),

    #[error("Error: {0}")]
    AnyhowError(#[from] anyhow::Error),

    #[error("Could not build request client: {0}")]
    BuildReqwestClientError(#[source] reqwest::Error),

    #[error("Could not parse header value: {0}")]
    ParsingHeaderValueError(#[source] reqwest::header::InvalidHeaderValue),
}
