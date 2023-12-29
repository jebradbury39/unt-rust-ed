
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
    #[error("Missing target wasm32-unknown-unknown, can install using `rustup target add wasm32-unknown-unknown`")]
    MissingCargoTargetInstallation,
    #[error("Hit unknown cargo build error.\nSTDOUT:\n{0}\nSTDERR:\n{1}")]
    UnknownCargoError(String, String),
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
