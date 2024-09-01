use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("io error {source:?}")]
    IoError {
        #[from]
        source: std::io::Error,
    },

    #[error("serde error {source:?}")]
    SerdeError {
        #[from]
        source: serde_json::Error,
    },

    #[error("reqwest error {source:?}")]
    ReqwestError {
        #[from]
        source: reqwest::Error,
    },

    #[error("parse error")]
    ParseError(String),

    #[error("TODO error")]
    Todo(String),
}
