#![cfg(test)]

use crate::response::{Response, Message};

impl Response {
    pub(crate) fn canonical(mut self) -> Response {
        self.canonicalise();
        self
    }
    
    fn canonicalise(&mut self) {
        if let Some(ref mut returns) = self.returns {
            returns.canonicalise();
        }
        for p in self.sends.iter_mut() {
            p.1.canonicalise();
        }
        self.sends.sort_by_key(|p| p.0);
    }
}

impl Message {
    fn canonicalise(&mut self) {
        match self {
            Message::ListRooms(rooms) => {
                rooms.sort_by_key(|r| r.0);
            },
            _ => {},
        }
    }
}
