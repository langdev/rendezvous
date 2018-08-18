use failure::{Backtrace, Fail};
use irc::error::IrcError;


#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Discord error: {}", _0)]
    Discord(String, Backtrace),

    #[fail(display = "Environment variable error: {}", _0)]
    EnvironmentVariable(#[cause] std::env::VarError),

    #[fail(display = "I/O error: {}", _0)]
    Io(#[cause] std::io::Error, Backtrace),

    #[fail(display = "IRC error: {}", _0)]
    Irc(#[cause] irc::error::IrcError, Backtrace),

    #[fail(display = "Fail to load the configuration: {}", _0)]
    Configuration(String, Backtrace),

    #[fail(display = "Some thread has poisoned unexpectedly")]
    UnexpectedlyPosioned(Backtrace),
}

impl Error {
    pub fn configuration<E: std::fmt::Display>(err: E) -> Self {
        Error::Configuration(err.to_string(), Backtrace::new())
    }
}

impl From<serenity::Error> for Error {
    fn from(err: serenity::Error) -> Self {
        Error::Discord(err.to_string(), Backtrace::new())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err, Backtrace::new())
    }
}

impl From<IrcError> for Error {
    fn from(err: IrcError) -> Self {
        Error::Irc(err, Backtrace::new())
    }
}

impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(_err: std::sync::PoisonError<T>) -> Self {
        Error::UnexpectedlyPosioned(Backtrace::new())
    }
}
