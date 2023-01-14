use std::sync::Arc;

use crate::models::{UserID, RoomID};

pub(crate) const SERVER_FULL: Message = Message::Error(Error::ServerFull);
pub(crate) const INVALID_REQUEST: Message = Message::Error(Error::InvalidRequest);

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Response {
    pub(crate) returns: Option<Message>,
    pub(crate) sends: Vec<(UserID, Message)>,
}

impl Response {
    pub(crate) fn empty() -> Response {
        Response {
            returns: None,
            sends: Vec::new(),
        }
    }
    
    pub(crate) fn returns(message: Message) -> Response {
        Response {
            returns: Some(message),
            sends: Vec::new(),
        }
    }
    
    pub(crate) fn error(e: Error) -> Response {
        Response::returns(Message::Error(e))
    }
    
    pub(crate) fn sends(user_id: UserID, message: Message) -> Response {
        Response::sends_all([
            (user_id, message)
        ])
    }
    
    pub(crate) fn sends_all<T: Into<Vec<(UserID, Message)>>>(messages: T) -> Response {
        Response {
            returns: None,
            sends: messages.into(),
        }
    }
}

pub(crate) type Result<T = Response> = std::result::Result<T, Error>;

impl From<Result> for Response {
    fn from(r: Result) -> Response {
        r.unwrap_or_else(Response::error)
    }
}
impl From<Result<()>> for Response {
    fn from(r: Result<()>) -> Response {
        r.map_or_else(
            Response::error,
            |_| Response::empty(),
        )
    }
}
impl FromIterator<(UserID, Message)> for Response {
    fn from_iter<T: IntoIterator<Item = (UserID, Message)>>(iter: T) -> Response {
        let messages: Vec<_> = iter.into_iter().collect();
        Response::sends_all(messages)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Message {
    Welcome(UserID),
    Pong(u32),
    ListRooms(Vec<(RoomID, Arc<str>)>),
    RoomCreated(RoomID),
    RoomJoined(RoomID),
    RoomClosed(RoomID),
    RoomRejected(RoomID, String),
    JoinRequested(RoomID, UserID, String),
    PlayerLeft(RoomID, UserID),
    ReceivedFrom(RoomID, UserID, String),
    ReceivedBroadcast(RoomID, Arc<str>),
    ReceivedIndividual(RoomID, String),
    Error(Error),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Error {
    ServerFull,
    InvalidRequest,
    AlreadyInARoom,
    AlreadyRequestedJoin,
    NotRoomOwner,
    IsRoomOwner,
    NotInThatRoom,
    NoSuchUser,
    NoSuchRoom,
    NoSuchJoinRequest,
}

impl From<Error> for Message {
    fn from(e: Error) -> Message {
        Message::Error(e)
    }
}
impl From<Error> for Response {
    fn from(e: Error) -> Response {
        Response::returns(e.into())
    }
}
impl From<Message> for Response {
    fn from(m: Message) -> Response {
        Response::returns(m)
    }
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::Welcome(user_id) => {
                write!(f, "WELCOME|{user_id}")
            },
            Message::Pong(sequence_number) => {
                write!(f, "PONG|{sequence_number}")
            },
            Message::ListRooms(rooms) => if rooms.is_empty() {
                write!(f, "NO_OPEN_GAMES")
            } else {
                write!(f, "OPEN_GAMES")?;
                for (room_id, data) in rooms {
                    write!(f, "|{room_id}|{data}")?;
                }
                Ok(())
            },
            Message::RoomCreated(room_id) => {
                write!(f, "CREATED_GAME|{room_id}")
            },
            Message::RoomJoined(room_id) => {
                write!(f, "JOINED|{room_id}")
            },
            Message::RoomClosed(room_id) => {
                write!(f, "GAME_OVER|{room_id}")
            },
            Message::RoomRejected(room_id, reason) => {
                write!(f, "REJECTED|{room_id}|{reason}")
            },
            Message::JoinRequested(room_id, user_id, msg) => {
                write!(f, "PLAYER_JOINED|{room_id}|{user_id}|{msg}")
            },
            Message::PlayerLeft(room_id, user_id) => {
                write!(f, "PLAYER_LEFT|{room_id}|{user_id}")
            },
            Message::ReceivedBroadcast(room_id, payload) => {
                write!(f, "RECEIVED|{room_id}|{payload}")
            },
            Message::ReceivedIndividual(room_id, payload) => {
                write!(f, "RECEIVED|{room_id}|{payload}")
            },
            Message::ReceivedFrom(room_id, user_id, payload) => {
                write!(f, "RECEIVED|{room_id}|{user_id}|{payload}")
            },
            Message::Error(e) => {
                write!(f, "ERROR|{e}")
            },
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ServerFull => f.write_str("Server is full"),
            Error::InvalidRequest => f.write_str("Invalid request"),
            Error::AlreadyInARoom => f.write_str("Already in a game"),
            Error::AlreadyRequestedJoin => f.write_str("Already requested to join a game"),
            Error::NotRoomOwner => f.write_str("You are not the game owner"),
            Error::IsRoomOwner => f.write_str("You are the game owner"),
            Error::NotInThatRoom => f.write_str("You are not in that game"),
            Error::NoSuchUser => f.write_str("No such user"),
            Error::NoSuchRoom => f.write_str("No such game"),
            Error::NoSuchJoinRequest => f.write_str("No such join request"),
        }
    }
}
