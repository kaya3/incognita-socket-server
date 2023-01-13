use std::collections::HashMap;
use std::sync::Arc;

use crate::request::Request;
use crate::response::{Error, Message, Response, Result};
use crate::models::{UserID, RoomID, User, Room, UserState};

fn next_id<T>(last_id: u32, map: &HashMap<u32, T>) -> u32 {
    let mut id = last_id;
    loop {
        id = id.wrapping_add(1);
        if !map.contains_key(&id) { return id; }
    }
}

#[derive(Default)]
pub(crate) struct Server {
    max_connections: usize,
    last_user_id: UserID,
    users: HashMap<UserID, User>,
    last_room_id: RoomID,
    rooms: HashMap<RoomID, Room>,
}

impl Server {
    pub(crate) fn new(max_connections: usize) -> Server {
        Server {
            max_connections,
            ..Default::default()
        }
    }
    
    fn get_user_mut(&mut self, user_id: UserID) -> Result<&mut User> {
        self.users.get_mut(&user_id)
            .ok_or(Error::NoSuchUser)
    }
    
    fn get_room(&self, room_id: RoomID) -> Result<&Room> {
        self.rooms.get(&room_id)
            .ok_or(Error::NoSuchRoom)
    }
    
    fn get_room_mut(&mut self, room_id: RoomID) -> Result<&mut Room> {
        self.rooms.get_mut(&room_id)
            .ok_or(Error::NoSuchRoom)
    }
    
    fn get_user_room_mut(&mut self, user_id: UserID, room_id: RoomID) -> Result<(&mut User, &mut Room)> {
        let user = self.users.get_mut(&user_id)
            .ok_or(Error::NoSuchUser)?;
        let room = self.rooms.get_mut(&room_id)
            .ok_or(Error::NoSuchRoom)?;
        Ok((user, room))
    }
    
    fn close_room(&mut self, room_id: RoomID) -> Result {
        let room = self.rooms.remove(&room_id)
            .ok_or(Error::NoSuchRoom)?;
        let all_users = room.members.into_iter()
            .chain(room.join_requests);
        
        if let Ok(owner) = self.get_user_mut(room.owner_id) {
            owner.state = UserState::Nowhere;
        }
        
        let mut messages = Vec::new();
        for u_id in all_users {
            let u = self.get_user_mut(u_id)?;
            u.state = UserState::Nowhere;
            messages.push((u_id, Message::RoomClosed(room_id)));
        }
        Ok(Response::sends_all(messages))
    }
    
    pub(crate) fn add_user(&mut self) -> Option<UserID> {
        if self.users.len() >= self.max_connections {
            return None;
        }
        
        let user_id = next_id(self.last_user_id, &self.users);
        let user = User::new(user_id);
        self.users.insert(user_id, user);
        self.last_user_id = user_id;
        Some(user_id)
    }
    
    pub(crate) fn remove_user(&mut self, user_id: UserID) -> Result {
        let mut user = self.users.remove(&user_id)
            .ok_or(Error::NoSuchUser)?;
        
        match user.state {
            UserState::RoomOwner(room_id) => {
                self.close_room(room_id)
            },
            UserState::InRoom(room_id) => {
                let room = self.get_room_mut(room_id)?;
                room.remove_user(user_id)?;
                let msg = Message::PlayerLeft(room_id, user_id);
                Ok(Response::sends(room.owner_id, msg))
            },
            UserState::RequestedJoin(room_id) => {
                let room = self.get_room_mut(room_id)?;
                room.cancel_join_request(&mut user)?;
                let msg = Message::PlayerLeft(room_id, user_id);
                Ok(Response::sends(room.owner_id, msg))
            },
            UserState::Nowhere => Ok(Response::empty(),)
        }
    }
    
    fn list_rooms(&self) -> Response {
        let rooms = self.rooms
            .values()
            .map(|room| (room.id, room.data.clone()))
            .collect();
        Message::ListRooms(rooms).into()
    }
    
    fn create_room(&mut self, user_id: UserID, data: String) -> Result {
        let room_id = next_id(self.last_room_id, &self.rooms);
        let room = self.get_user_mut(user_id)?
            .try_create_room(room_id, data)?;
        self.rooms.insert(room_id, room);
        self.last_room_id = room_id;
        Ok(Message::RoomCreated(room_id).into())
    }
    
    fn ask_join(&mut self, user_id: UserID, room_id: RoomID, msg: String) -> Result {
        let (user, room) = self.get_user_room_mut(user_id, room_id)?;
        user.try_join_room(room)?;
        Ok(Response::sends(room.owner_id, Message::JoinRequested(room_id, user.id, msg)))
    }
    
    fn accept_join(&mut self, user_id: UserID, room_id: RoomID, other_id: UserID) -> Result {
        let (other, room) = self.get_user_room_mut(other_id, room_id)?;
        room.expect_owner(user_id)?;
        room.accept_join_request(other)?;
        
        Ok(Response::sends(other_id, Message::RoomJoined(room_id)))
    }
    
    fn reject_join(&mut self, user_id: UserID, room_id: RoomID, other_id: UserID, reason: String) -> Result {
        let (other, room) = self.get_user_room_mut(other_id, room_id)?;
        room.expect_owner(user_id)?;
        room.cancel_join_request(other)?;
        Ok(Response::sends(other_id, Message::RoomRejected(room_id, reason)))
    }
    
    fn leave_room(&mut self, user_id: UserID, room_id: RoomID) -> Result {
        let (user, room) = self.get_user_room_mut(user_id, room_id)?;
        
        if room.owner_id == user.id {
            self.close_room(room_id)
        } else {
            user.leave_room(room)?;
            Ok(Response::sends(room.owner_id, Message::PlayerLeft(room_id, user.id)))
        }
    }
    
    fn send(&self, from_user_id: UserID, room_id: RoomID, payload: String) -> Result {
        let room = self.get_room(room_id)?;
        
        Ok(if from_user_id == room.owner_id {
            let payload: Arc<str> = Arc::from(payload);
            room.members.iter()
                .copied()
                .map(|u_id| (u_id, Message::ReceivedBroadcast(room_id, payload.clone())))
                .collect()
        } else {
            let message = Message::ReceivedFrom(room_id, from_user_id, payload);
            Response::sends(room.owner_id, message)
        })
    }
    
    fn send_to(&self, from_user_id: UserID, room_id: RoomID, to_user_id: UserID, payload: String) -> Result {
        let room = self.get_room(room_id)?;
        room.expect_owner(from_user_id)?;
        room.expect_member(to_user_id)?;
        
        let message = Message::ReceivedIndividual(room_id, payload);
        Ok(Response::sends(to_user_id, message))
    }
    
    fn echo_from(&self, user_id: UserID, room_id: RoomID, from_user_id: UserID, payload: String) -> Result {
        let room = self.get_room(room_id)?;
        room.expect_owner(user_id)?;
        room.expect_member(from_user_id)?;
        
        let payload: Arc<str> = Arc::from(payload);
        Ok(room.members.iter()
            .copied()
            .filter(|&u_id| u_id != from_user_id)
            .map(|u_id| (u_id, Message::ReceivedBroadcast(room_id, payload.clone())))
            .collect())
    }
    
    pub(crate) fn handle_request(&mut self, user_id: UserID, request: Request) -> Response {
        match request {
            Request::ListRooms => {
                self.list_rooms()
            },
            Request::CreateRoom(data) => {
                self.create_room(user_id, data).into()
            },
            Request::AskJoinRoom(room_id, msg) => {
                self.ask_join(user_id, room_id, msg).into()
            },
            Request::AcceptJoinRoom(room_id, other_id) => {
                self.accept_join(user_id, room_id, other_id).into()
            },
            Request::RejectJoinRoom(room_id, other_id, reason) => {
                self.reject_join(user_id, room_id, other_id, reason).into()
            },
            Request::LeaveRoom(room_id) => {
                self.leave_room(user_id, room_id).into()
            },
            Request::Send(room_id, payload) => {
                self.send(user_id, room_id, payload).into()
            },
            Request::SendTo(room_id, other_id, payload) => {
                self.send_to(user_id, room_id, other_id, payload).into()
            },
            Request::EchoFrom(room_id, other_id, payload) => {
                self.echo_from(user_id, room_id, other_id, payload).into()
            },
            Request::Quit => {
                Response::empty()
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    
    fn ok(t: Message) -> Result {
        Ok(t.into())
    }
    
    #[test]
    fn add_user() {
        let mut server = Server::new(4);
        assert_eq!(Some(1), server.add_user());
    }
    
    #[test]
    fn remove_user() {
        let mut server = Server::new(4);
        assert_eq!(Some(1), server.add_user());
        assert_eq!(Ok(Response::empty()), server.remove_user(1));
    }
    
    #[test]
    fn max_connections() {
        let mut server = Server::new(4);
        assert_eq!(Some(1), server.add_user());
        assert_eq!(Some(2), server.add_user());
        assert_eq!(Some(3), server.add_user());
        assert_eq!(Some(4), server.add_user());
        assert_eq!(None, server.add_user());
    }
    
    #[test]
    fn create_room() {
        let mut server = Server::new(4);
        server.add_user().unwrap();
        assert_eq!(ok(Message::RoomCreated(1)), server.create_room(1, "hello".into()));
    }
    
    #[test]
    fn list_rooms() {
        let mut server = Server::new(4);
        server.add_user().unwrap();
        server.add_user().unwrap();
        
        assert_eq!(ok(Message::RoomCreated(1)), server.create_room(2, "hello".into()));
        assert_eq!(ok(Message::RoomCreated(2)), server.create_room(1, "world".into()));
        
        let expected: Response = Message::ListRooms(vec![
            (1, "hello".into()),
            (2, "world".into()),
        ]).into();
        assert_eq!(expected, server.list_rooms().canonical());
    }
    
    #[test]
    fn ask_join() {
        let mut server = Server::new(4);
        server.add_user().unwrap();
        server.add_user().unwrap();
        server.create_room(1, "hello".into()).unwrap();
        
        let expected = Response::sends(1, Message::JoinRequested(1, 2, "please".into()));
        assert_eq!(Ok(expected), server.ask_join(2, 1, "please".into()));
    }
    
    #[test]
    fn accept_join() {
        let mut server = Server::new(4);
        server.add_user().unwrap();
        server.add_user().unwrap();
        server.create_room(1, "hello".into()).unwrap();
        server.ask_join(2, 1, "please".into()).unwrap();
        
        let expected = Response::sends(2, Message::RoomJoined(1));
        assert_eq!(Ok(expected), server.accept_join(1, 1, 2));
    }
    
    #[test]
    fn reject_join() {
        let mut server = Server::new(4);
        server.add_user().unwrap();
        server.add_user().unwrap();
        server.create_room(1, "hello".into()).unwrap();
        server.ask_join(2, 1, "please".into()).unwrap();
        
        let expected = Response::sends(2, Message::RoomRejected(1, "no".into()));
        assert_eq!(Ok(expected), server.reject_join(1, 1, 2, "no".into()));
    }
    
    #[test]
    fn leave_room() {
        let mut server = Server::new(4);
        server.add_user().unwrap();
        server.add_user().unwrap();
        server.create_room(1, "hello".into()).unwrap();
        server.ask_join(2, 1, "please".into()).unwrap();
        server.accept_join(1, 1, 2).unwrap();
        
        let expected = Response::sends(1, Message::PlayerLeft(1, 2));
        assert_eq!(Ok(expected), server.leave_room(2, 1));
    }
    
    #[test]
    fn close_room() {
        let mut server = Server::new(4);
        server.add_user().unwrap();
        server.add_user().unwrap();
        server.create_room(1, "hello".into()).unwrap();
        server.ask_join(2, 1, "please".into()).unwrap();
        server.accept_join(1, 1, 2).unwrap();
        
        let expected = Response::sends(2, Message::RoomClosed(1));
        assert_eq!(Ok(expected), server.leave_room(1, 1));
    }
    
    #[test]
    fn owner_send() {
        let mut server = Server::new(4);
        server.add_user().unwrap();
        server.add_user().unwrap();
        server.add_user().unwrap();
        server.create_room(1, "hello".into()).unwrap();
        server.ask_join(2, 1, "please".into()).unwrap();
        server.ask_join(3, 1, "please".into()).unwrap();
        server.accept_join(1, 1, 2).unwrap();
        server.accept_join(1, 1, 3).unwrap();
        
        let expected = Response::sends_all([
            (2, Message::ReceivedBroadcast(1, "whee".into())),
            (3, Message::ReceivedBroadcast(1, "whee".into())),
        ]);
        assert_eq!(Ok(expected), server.send(1, 1, "whee".into()));
    }
    
    #[test]
    fn member_send() {
        let mut server = Server::new(4);
        server.add_user().unwrap();
        server.add_user().unwrap();
        server.create_room(1, "hello".into()).unwrap();
        server.ask_join(2, 1, "please".into()).unwrap();
        server.accept_join(1, 1, 2).unwrap();
        
        let expected = Response::sends(1, Message::ReceivedFrom(1, 2, "whee".into()));
        assert_eq!(Ok(expected), server.send(2, 1, "whee".into()));
    }
    
    #[test]
    fn send_to() {
        let mut server = Server::new(4);
        server.add_user().unwrap();
        server.add_user().unwrap();
        server.add_user().unwrap();
        server.create_room(1, "hello".into()).unwrap();
        server.ask_join(2, 1, "please".into()).unwrap();
        server.ask_join(3, 1, "please".into()).unwrap();
        server.accept_join(1, 1, 2).unwrap();
        server.accept_join(1, 1, 3).unwrap();
        
        let expected = Response::sends(2, Message::ReceivedIndividual(1, "whee".into()));
        assert_eq!(Ok(expected), server.send_to(1, 1, 2, "whee".into()));
    }
    
    #[test]
    fn echo_from() {
        let mut server = Server::new(4);
        server.add_user().unwrap();
        server.add_user().unwrap();
        server.add_user().unwrap();
        server.create_room(1, "hello".into()).unwrap();
        server.ask_join(2, 1, "please".into()).unwrap();
        server.ask_join(3, 1, "please".into()).unwrap();
        server.accept_join(1, 1, 2).unwrap();
        server.accept_join(1, 1, 3).unwrap();
        
        let expected = Response::sends(3, Message::ReceivedBroadcast(1, "whee".into()));
        assert_eq!(Ok(expected), server.echo_from(1, 1, 2, "whee".into()));
    }
    
    #[test]
    fn owner_quit_during_game() {
        let mut server = Server::new(4);
        server.add_user().unwrap();
        server.create_room(1, "hello".into()).unwrap();
        
        assert_eq!(Ok(Response::empty()), server.remove_user(1));
    }
    
    #[test]
    fn member_quit_during_game() {
        let mut server = Server::new(4);
        server.add_user().unwrap();
        server.add_user().unwrap();
        server.create_room(1, "hello".into()).unwrap();
        server.ask_join(2, 1, "please".into()).unwrap();
        server.accept_join(1, 1, 2).unwrap();
        
        let expected = Response::sends(1, Message::PlayerLeft(1, 2));
        assert_eq!(Ok(expected), server.remove_user(2));
    }
}
