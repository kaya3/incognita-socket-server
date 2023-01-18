use crate::models::{UserID, RoomID};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Request {
    ListRooms,
    Ping(u32),
    CreateRoom(String),
    SetOwner(RoomID, UserID),
    AskJoinRoom(RoomID, String),
    AcceptJoinRoom(RoomID, UserID),
    RejectJoinRoom(RoomID, UserID, String),
    LeaveRoom(RoomID),
    Send(RoomID, String),
    SendTo(RoomID, UserID, String),
    EchoFrom(RoomID, UserID, String),
    Quit,
}

impl Request {
    pub(crate) fn is_quit(&self) -> bool {
        matches!(self, Request::Quit)
    }
}

struct Parts<'a> (std::str::Split<'a, char>);
impl <'a> Parts<'a> {
    fn of(s: &'a str) -> Parts<'a> {
        Parts(s.split('|'))
    }
    
    fn take_str(&mut self) -> Option<&str> {
        self.0.next()
    }
    
    fn take_string(&mut self) -> Option<String> {
        self.0.next()
            .map(str::to_string)
    }
    
    fn take_int<T: std::str::FromStr>(&mut self) -> Option<T> {
        self.0.next()
            .and_then(|s| s.parse::<T>().ok())
    }
    
    fn done(self, then: impl FnOnce() -> Request) -> Option<Request> {
        (self.0.count() == 0).then(then)
    }
}

pub(crate) fn parse(s: &str) -> Option<Request> {
    let mut parts = Parts::of(s);
    match parts.take_str()? {
        "LIST_OPEN_GAMES" => {
            parts.done(|| Request::ListRooms)
        },
        "PING" => {
            let sequence_number = parts.take_int()?;
            parts.done(|| Request::Ping(sequence_number))
        },
        "CREATE_GAME" => {
            let data = parts.take_string()?;
            parts.done(|| Request::CreateRoom(data))
        },
        "SET_OWNER" => {
            let room_id = parts.take_int()?;
            let other_id = parts.take_int()?;
            parts.done(|| Request::SetOwner(room_id, other_id))
        },
        "JOIN_GAME" => {
            let room_id = parts.take_int()?;
            let msg = parts.take_string()?;
            parts.done(|| Request::AskJoinRoom(room_id, msg))
        },
        "LEAVE_GAME" => {
            let room_id = parts.take_int()?;
            parts.done(|| Request::LeaveRoom(room_id))
        },
        "ACCEPT_JOIN" => {
            let room_id = parts.take_int()?;
            let user_id = parts.take_int()?;
            parts.done(|| Request::AcceptJoinRoom(room_id, user_id))
        },
        "REJECT_JOIN" => {
            let room_id = parts.take_int()?;
            let user_id = parts.take_int()?;
            let reason = parts.take_string()?;
            parts.done(|| Request::RejectJoinRoom(room_id, user_id, reason))
        },
        "SEND" => {
            let room_id = parts.take_int()?;
            let payload = parts.take_string()?;
            parts.done(|| Request::Send(room_id, payload))
        },
        "SEND_TO" => {
            let room_id = parts.take_int()?;
            let user_id = parts.take_int()?;
            let payload = parts.take_string()?;
            parts.done(|| Request::SendTo(room_id, user_id, payload))
        },
        "ECHO_FROM" => {
            let room_id = parts.take_int()?;
            let user_id = parts.take_int()?;
            let payload = parts.take_string()?;
            parts.done(|| Request::EchoFrom(room_id, user_id, payload))
        },
        "QUIT" => {
            parts.done(|| Request::Quit)
        },
        _ => None,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    
    #[test]
    fn list_rooms() {
        let r = parse("LIST_OPEN_GAMES").unwrap();
        assert_eq!(Request::ListRooms, r);
    }
    
    #[test]
    fn ping() {
        let r = parse("PING|23").unwrap();
        assert_eq!(Request::Ping(23), r);
    }
    
    #[test]
    fn create_room() {
        let r = parse("CREATE_GAME|hello").unwrap();
        assert_eq!(Request::CreateRoom("hello".into()), r);
    }
    
    #[test]
    fn set_owner() {
        let r = parse("SET_OWNER|1|2").unwrap();
        assert_eq!(Request::SetOwner(1, 2), r);
    }
    
    #[test]
    fn ask_join() {
        let r = parse("JOIN_GAME|3|hello").unwrap();
        assert_eq!(Request::AskJoinRoom(3, "hello".into()), r);
    }
    
    #[test]
    fn accept_join() {
        let r = parse("ACCEPT_JOIN|3|4").unwrap();
        assert_eq!(Request::AcceptJoinRoom(3, 4), r);
    }
    
    #[test]
    fn reject_join() {
        let r = parse("REJECT_JOIN|3|4|ur banned").unwrap();
        assert_eq!(Request::RejectJoinRoom(3, 4, "ur banned".into()), r);
    }   
    
    #[test]
    fn leave_room() {
        let r = parse("LEAVE_GAME|3").unwrap();
        assert_eq!(Request::LeaveRoom(3), r);
    }
    
    #[test]
    fn send() {
        let r = parse("SEND|3|hello").unwrap();
        assert_eq!(Request::Send(3, "hello".into()), r);
    }
    
    #[test]
    fn send_to() {
        let r = parse("SEND_TO|3|4|hello").unwrap();
        assert_eq!(Request::SendTo(3, 4, "hello".into()), r);
    }
    
    #[test]
    fn echo_from() {
        let r = parse("ECHO_FROM|3|4|hello").unwrap();
        assert_eq!(Request::EchoFrom(3, 4, "hello".into()), r);
    }
}
