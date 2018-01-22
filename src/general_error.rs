use super::file_error;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum ErrorKind {
    RSCodecCreateFail,
    FileError(file_error::FileError)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    kind : ErrorKind
}

impl fmt::Display for Error {
    fn fmt(&self, f : &mut fmt::Formatter) -> fmt::Result {
        use self::ErrorKind::*;
        match self.kind {
            RSCodecCreateFail => write!(f, "Reed-Solomon codec creation fail"),
            FileError(ref e)  => write!(f, "{}", e),
            _                 => write!(f, "Unknown error")
        }
    }
}