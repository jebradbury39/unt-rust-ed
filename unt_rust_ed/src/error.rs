
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UntRustedError {
    #[error("Io Error for {resource}: {err}")]
    IoError {
        resource: String,
        err: std::io::Error,
    },
    #[error("Extism error: {0}")]
    Extism(extism::Error),
    #[error("Serde reader error (resource={0}): {1}")]
    SerdeReader(String, flexbuffers::ReaderError),
    #[error("Serde error during deserialize (resource={0}): {1}")]
    SerdeDeserialize(String, flexbuffers::DeserializationError),
    #[error("Serde error during serialize (resource={0}): {1}")]
    SerdeSerialize(String, flexbuffers::SerializationError),
    #[error("Syn error: {0}")]
    Syn(syn::Error),
    #[error("Missing target {0}, can install using `rustup target add {0}`")]
    MissingCargoTargetInstallation(String),
    #[error("Hit unknown cargo build error.\nSTDOUT:\n{0}\nSTDERR:\n{1}")]
    UnknownCargoError(String, String),
    #[error("This PatType is not supported: {0}")]
    UnsupportedParamName(String),
    #[error("This FnArg is not supported: {0}")]
    UnsupportedFnArg(String),
    #[error("This external function call ({0}) took too long to execute")]
    RuntimeExceededTimeout(String),
    #[error("This external function call ({0}) attempted to allocate too much memory")]
    RuntimeExceededMemory(String),
    #[error("Cached compiled project hash did not match, so recompiling the project")]
    CachedHashMismatch,
}

impl From<extism::Error> for UntRustedError {
    fn from(err: extism::Error) -> Self {
        Self::Extism(err)
    }
}

impl From<syn::Error> for UntRustedError {
    fn from(err: syn::Error) -> Self {
        Self::Syn(err)
    }
}

pub type Result<T> = std::result::Result<T, UntRustedError>;
