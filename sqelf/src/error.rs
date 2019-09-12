use std::{
    error,
    fmt,
};

pub struct Error(Inner);

impl Error {
    pub fn msg(msg: impl fmt::Display) -> Self {
        err_msg(msg)
    }
}

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

impl error::Error for Inner {}

impl<E> From<E> for Error
where
    E: error::Error,
{
    fn from(err: E) -> Error {
        Error(Inner(err.to_string()))
    }
}

impl From<Error> for Box<dyn error::Error + Send + Sync> {
    fn from(err: Error) -> Box<dyn error::Error + Send + Sync> {
        Box::new(err.0)
    }
}

impl From<Error> for Box<dyn error::Error> {
    fn from(err: Error) -> Box<dyn error::Error> {
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
