use super::Error;
use std::fmt::Display;

pub trait ErrorContext<T> {
    fn context<D: Display>(self, msg: D) -> Result<T, Error>;

    fn with_context<D: Display, F: FnOnce() -> D>(self, f: F) -> Result<T, Error>;
}

impl<T, E> ErrorContext<T> for Result<T, E>
where
    Error: From<E>,
{
    fn context<D: Display>(self, msg: D) -> Result<T, Error> {
        self.map_err(|err| {
            let mut err = Error::from(err);
            err.context = Some(msg.to_string());
            err
        })
    }

    fn with_context<D: Display, F: FnOnce() -> D>(self, f: F) -> Result<T, Error> {
        self.map_err(|err| {
            let mut err = Error::from(err);
            err.context = Some(f().to_string());
            err
        })
    }
}
