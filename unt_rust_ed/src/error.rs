
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
