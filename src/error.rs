
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
}

impl From<extism::Error> for UntRustedError {
    fn from(err: extism::Error) -> Self {
        Self::Extism(err)
    }
}

pub type Result<T> = std::result::Result<T, UntRustedError>;
