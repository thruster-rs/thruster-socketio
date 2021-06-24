use chashmap::{CHashMap, ReadGuard};
use log::{debug};

use crate::socketio::InternalMessage;

// use crossbeam::channel::Sender;
use tokio::sync::broadcast::Sender;

lazy_static! {
    static ref ROOMS: CHashMap<String, Vec<ChannelPair>> = CHashMap::new();
}

// pub type ChannelPair = Sender<SocketIOMessage>;
pub struct ChannelPair {
    sid: String,
    sender: Sender<InternalMessage>,
}

impl ChannelPair {
    pub fn new(sid: &str, sender: Sender<InternalMessage>) -> Self {
        ChannelPair {
            sid: sid.to_string(),
            sender,
        }
    }

    pub fn send(&self, message: InternalMessage) {
        let _ = self.sender.send(message);
    }

    pub fn sid(&self) -> &str {
        &self.sid
    }
}

pub fn join_channel_to_room(room_id: &str, channel_pair: ChannelPair) {
    let mut connected_sockets = match ROOMS.remove(room_id) {
        Some(val) => val,
        None => Vec::new(),
    };

    connected_sockets.push(channel_pair);

    ROOMS.insert(room_id.to_string(), connected_sockets);
    
    //test
    debug!("ROOMS join_channel_to_room, room_id = {}", room_id);
    match ROOMS.get(room_id) {
        Some(sockets) => {
            for socket in &*sockets {
                debug!("ROOMS join_channel_to_room, readGuard socket id = {}", socket.sid());
            }
        }
        None => ()
    };
}

pub fn remove_socket_from_room(room_id: &str, _sid: &str) {
    let mut connected_sockets = match ROOMS.remove(room_id) {
        Some(val) => val,
        None => Vec::new(),
    };

    for i in 0..connected_sockets.len() - 1 {
        let i = connected_sockets.len() - 1 - i;
        let socket = connected_sockets.get(i).unwrap();

        if socket.sid == room_id {
            connected_sockets.remove(i);
            break;
        }
    }

    ROOMS.insert(room_id.to_string(), connected_sockets);

    //test
    debug!("ROOMS remove_socket_from_room, room_id = {}, sid = {}", room_id, _sid);
    match ROOMS.get(room_id) {
        Some(sockets) => {
            for socket in &*sockets {
                debug!("ROOMS remove_socket_from_room, readGuard socket id = {}", socket.sid());
            }
        }
        None => ()
    };
}

pub fn get_sockets_for_room(room_id: &str) -> Option<ReadGuard<String, Vec<ChannelPair>>> {
    ROOMS.get(room_id)
}

pub fn sockets_number(room_id: &str) -> usize {
    match ROOMS.get(room_id) {
        Some(channels) => {
            return channels.len();
        }
        None => ()
    }

    0
}