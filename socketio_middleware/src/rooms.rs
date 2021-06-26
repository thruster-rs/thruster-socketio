use chashmap::{CHashMap, ReadGuard};
use log::{debug};

use crate::socketio::InternalMessage;

// use crossbeam::channel::Sender;
use tokio::sync::broadcast::Sender;

lazy_static! {
    static ref ROOMS: CHashMap<String, Vec<ChannelPair>> = CHashMap::new();
}

// pub type ChannelPair = Sender<SocketIOMessage>;
#[derive(Clone)]
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
    //TODO: why use remove, it is low performance.
    let mut connected_sockets = match ROOMS.remove(room_id) {
        Some(val) => val,
        None => Vec::new(),
    };

    //check if socketid exist
    let mut exist = false;
    for socket in &connected_sockets {
        if socket.sid() == channel_pair.sid() {
            debug!("ROOMS: socketid {} doesn't join room {}, this socketid already exist in the room.", channel_pair.sid(), room_id);
            exist = true;
            break;
        }
    }

    if !exist {
        debug!("ROOMS: socketid {} joined room {}.", channel_pair.sid(), room_id);
        connected_sockets.push(channel_pair);
    }
    ROOMS.insert(room_id.to_string(), connected_sockets);
}

pub fn remove_socket_from_room(room_id: &str, sid: &str) {
    let mut connected_sockets = match ROOMS.remove(room_id) {
        Some(val) => val,
        None => Vec::new(),
    };

    //check if socketid exist
    let mut i = 0;
    for socket in &connected_sockets {
        if socket.sid() == sid {
            debug!("ROOMS: socketid {} leaved room {}.", socket.sid(), room_id);
            connected_sockets.remove(i);
            break;
        }

        i += 1;
    }

    ROOMS.insert(room_id.to_string(), connected_sockets);
}

pub fn get_sockets_for_room(room_id: &str) -> Option<ReadGuard<String, Vec<ChannelPair>>> {
    ROOMS.get(room_id)
}

///
/// get sockets number for room
///
pub fn get_sockets_number_for_room(room_id: &str) -> usize {
    ROOMS.get(room_id).map(|channels| channels.len()).unwrap_or(0)
}

///
/// print all sockets for room
///
pub fn print_sockets_for_room(room_id: &str) {
    match ROOMS.get(room_id) {
        Some(sockets) => {
            for socket in &*sockets {
                debug!("ROOMS: room {} containted socketid {}, sockets number = {}.", room_id, socket.sid(), sockets.len());
            }
        }

        None => {
            debug!("ROOMS: no socket in room {}.", room_id);
        }
    }
}
