use async_std::{io, task};
use futures::channel::mpsc;

use crate::response;

pub(crate) type Result = std::result::Result<(), ServerError>;

#[derive(Debug)]
pub(crate) enum ServerError {
    IO(io::Error),
    InvalidState(response::Error),
    DispatcherFailed(mpsc::SendError),
}

impl From<io::Error> for ServerError {
    fn from(e: io::Error) -> ServerError {
        ServerError::IO(e)
    }
}
impl From<response::Error> for ServerError {
    fn from(e: response::Error) -> ServerError {
        ServerError::InvalidState(e)
    }
}
impl From<mpsc::SendError> for ServerError {
    fn from(e: mpsc::SendError) -> ServerError {
        Self::DispatcherFailed(e)
    }
}

pub(crate) fn spawn_logged_task<F>(fut: F) -> task::JoinHandle<()> where F: futures::Future<Output = Result> + Send + 'static {
    task::spawn(async move {
        if let Err(e) = fut.await {
            eprintln!("SERVER ERROR: {e:?}")
        }
    })
}
