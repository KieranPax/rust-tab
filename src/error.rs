#[derive(Debug)]
pub enum Error {
    IOError(std::io::Error),
    NoEvent,
    MalformedCmd(String),
    UnknownCmd(String),
    InvalidOp(String),
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
