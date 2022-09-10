#[derive(Debug)]
pub enum Error {
    IOError(std::io::Error),
    NoEvent,
    MalformedCmd(String),
    UnknownCmd(String),
    InvalidOp(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IOError(arg0) => write!(f, "IOError({arg0:?})"),
            Self::NoEvent => write!(f, "NoEvent"),
            Self::MalformedCmd(arg0) => write!(f, "MalformedCmd({arg0})"),
            Self::UnknownCmd(arg0) => write!(f, "UnknownCmd({arg0})"),
            Self::InvalidOp(arg0) => write!(f, "InvalidOp({arg0})"),
        }
    }
}
pub type Result<T> = std::result::Result<T, Error>;

mod error {
    #[macro_export]
    macro_rules! map_io_err {
        ($code:expr) => {
            $code.map_err(|e| crate::error::Error::IOError(e))
        };
    }
}
