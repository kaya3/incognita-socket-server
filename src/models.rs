use std::sync::Arc;

use crate::response::{Error, Result};

pub(crate) type UserID = u32;
pub(crate) type RoomID = u32;

#[derive(Debug)]
pub(crate) struct User {
    pub(crate) id: UserID,
    pub(crate) state: UserState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UserState {
    RoomOwner(RoomID),
    InRoom(RoomID),
    RequestedJoin(RoomID),
    Nowhere,
}

impl Default for UserState {
    fn default() -> UserState {
        UserState::Nowhere
    }
}

#[derive(Debug)]
pub(crate) struct Room {
    pub(crate) id: RoomID,
    pub(crate) owner_id: UserID,
    pub(crate) data: Arc<str>,
    pub(crate) members: Vec<UserID>,
    pub(crate) join_requests: Vec<UserID>,
}

impl User {
    pub(crate) fn new(id: UserID) -> User {
        User {
            id,
            state: UserState::Nowhere,
        }
    }
    
    fn expect_nowhere(&self) -> Result<()> {
        match self.state {
            UserState::RoomOwner(_) |
            UserState::InRoom(_) => Err(Error::AlreadyInARoom),
            UserState::RequestedJoin(_) => Err(Error::AlreadyRequestedJoin),
            UserState::Nowhere => Ok(()),
        }
    }
    
    pub(crate) fn try_create_room(&mut self, room_id: RoomID, data: String) -> Result<Room> {
        self.expect_nowhere()?;
        let room = Room::new(room_id, self.id, data);
        self.state = UserState::RoomOwner(room_id);
        Ok(room)
    }
    
    pub(crate) fn try_join_room(&mut self, room: &mut Room) -> Result<()> {
        self.expect_nowhere()?;
        self.state = UserState::RequestedJoin(room.id);
        room.join_requests.push(self.id);
        Ok(())
    }
    
    pub(crate) fn leave_room(&mut self, room: &mut Room) -> Result<()> {
        match self.state {
            UserState::RoomOwner(_) => {
                Err(Error::IsRoomOwner)
            },
            UserState::InRoom(room_id) => {
                if room_id == room.id {
                    self.state = UserState::Nowhere;
                    room.remove_user(self.id)
                } else {
                    Err(Error::NotInThatRoom)
                }
            },
            UserState::RequestedJoin(room_id) => {
                if room_id == room.id {
                    room.cancel_join_request(self)
                } else {
                    Err(Error::NotInThatRoom)
                }
            },
            UserState::Nowhere => {
                Err(Error::NotInThatRoom)
            },
        }
    }
}

impl Room {
    pub(crate) fn new(id: RoomID, owner_id: UserID, data: String) -> Room {
        Room {
            id,
            owner_id,
            data: Arc::from(data),
            members: Vec::new(),
            join_requests: Vec::new(),
        }
    }
    
    pub(crate) fn expect_owner(&self, user_id: UserID) -> Result<()> {
        if self.owner_id == user_id {
            Ok(())
        } else {
            Err(Error::NotRoomOwner)
        }
    }
    
    pub(crate) fn expect_member(&self, user_id: UserID) -> Result<()> {
        if self.members.contains(&user_id) {
            Ok(())
        } else {
            Err(Error::NoSuchUser)
        }
    }
    
    pub(crate) fn set_owner(&mut self, user: &mut User) -> Result<()> {
        let index = index_of(&self.members, user.id, Error::NoSuchUser)?;
        std::mem::swap(&mut self.members[index], &mut self.owner_id);
        user.state = UserState::RoomOwner(self.id);
        Ok(())
    }
    
    pub(crate) fn cancel_join_request(&mut self, user: &mut User) -> Result<()> {
        let index = index_of(&self.join_requests, user.id, Error::NoSuchJoinRequest)?;
        self.join_requests.swap_remove(index);
        user.state = UserState::Nowhere;
        Ok(())
    }
    
    pub(crate) fn accept_join_request(&mut self, user: &mut User) -> Result<()> {
        self.cancel_join_request(user)?;
        
        self.members.push(user.id);
        user.state = UserState::InRoom(self.id);
        Ok(())
    }
    
    pub(crate) fn remove_user(&mut self, user_id: UserID) -> Result<()> {
        if user_id == self.owner_id {
            return Err(Error::IsRoomOwner);
        }
        
        let index = index_of(&self.members, user_id, Error::NoSuchUser)?;
        self.members.swap_remove(index);
        Ok(())
    }
}

fn index_of<T: Eq>(arr: &[T], v: T, e: Error) -> Result<usize> {
    arr.iter()
        .position(|t| *t == v)
        .ok_or(e)
}
