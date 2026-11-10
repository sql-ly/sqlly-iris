#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid IRIS configuration: {0}")]
    InvalidConfig(String),

    #[error("IRIS protocol error: {0}")]
    Protocol(String),

    #[error("IRIS server error {code}: {message}")]
    Server { code: i16, message: String },

    #[error("expected {expected} SQL parameters, received {actual}")]
    ParameterCount { expected: usize, actual: usize },

    #[error("unsupported IRIS capability: {0}")]
    Unsupported(&'static str),

    #[error("the IRIS connection is unusable: {0}")]
    ConnectionUnusable(&'static str),

    #[error("column index {0} is out of range")]
    ColumnIndex(usize),

    #[error("unknown column {0:?}")]
    ColumnName(String),

    #[error("TLS error: {0}")]
    Tls(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
