use std::{
    fmt,
    error,
};

pub(crate) type StdError = Box<error::Error + Send + Sync>;

pub struct Error(Inner);

struct Inner(String);

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for Inner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl error::Error for Inner {

}

impl<E> From<E> for Error
where
    E: error::Error,
{
    fn from(err: E) -> Error {
        Error(Inner(err.to_string()))
    }
}

impl From<Error> for StdError {
    fn from(err: Error) -> StdError {
        Box::new(err.0)
    }
}

pub(crate) fn err_msg(msg: impl fmt::Display) -> Error {
    Error(Inner(msg.to_string()))
}

macro_rules! bail {
    ($($msg:tt)*) => {
        Err($crate::error::err_msg(format_args!($($msg)*)))?
    };
}