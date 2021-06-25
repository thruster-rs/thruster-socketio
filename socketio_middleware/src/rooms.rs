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
    let mut exist: bool = false;
    for socket in &connected_sockets {
        if socket.sid() == channel_pair.sid() {
            debug!("ROOMS: socketid {} doesn't join into room {}, this socketid already exist in the room.", channel_pair.sid(), room_id);
            exist = true;
            break;
        }
    }

    if false == exist {
        debug!("ROOMS: socketid {} joined into room {}.", channel_pair.sid(), room_id);
        connected_sockets.push(channel_pair);
    }
    ROOMS.insert(room_id.to_string(), connected_sockets);

    //print all sockets in the room
    match ROOMS.get(room_id) {
        Some(sockets) => {
            for socket in &*sockets {
                debug!("ROOMS: room {} containted socketid {}.", room_id, socket.sid());
            }
        }
        None => {
            debug!("ROOMS: no socketid in room {}.", room_id);
        }
    }
}

/*
pub fn join_channel_to_room(room_id: &str, channel_pair: ChannelPair) {
    match ROOMS.get_mut(room_id) {
        Some(mut connected_sockets) => {
            //check if socketid exist
            for socket in &*connected_sockets {
                debug!("In room {}, socket_id = {}", room_id, socket.sid());
                if socket.sid() == channel_pair.sid() {
                    debug!("Join channel to room {}, socket_id({}) is exist.", room_id, socket.sid());
                    //return;
                }
            }

            debug!("Socket_id {} insert a exist room {}", channel_pair.sid(), room_id);
            connected_sockets.push(channel_pair);
            ROOMS.insert(room_id.to_string(), connected_sockets.to_vec());
        }
        None => {
            debug!("Socket_id {} insert a new room {}", channel_pair.sid(), room_id);

            let mut connected_sockets = Vec::new();
            connected_sockets.push(channel_pair);
            ROOMS.insert(room_id.to_string(), connected_sockets);
        }
    };
}
*/

pub fn remove_socket_from_room(room_id: &str, sid: &str) {
    let mut connected_sockets = match ROOMS.remove(room_id) {
        Some(val) => val,
        None => Vec::new(),
    };

    //check if socketid exist
    let mut i = 0;
    for socket in &connected_sockets {
        if socket.sid() == sid {
            debug!("ROOMS: socketid {} leaved from room {}.", socket.sid(), room_id);
            connected_sockets.remove(i);
            break;
        }

        i += 1;
    }

    ROOMS.insert(room_id.to_string(), connected_sockets);

    //print all sockets in the room
    match ROOMS.get(room_id) {
        Some(sockets) => {
            for socket in &*sockets {
                debug!("ROOMS: room {} containted socketid {}.", room_id, socket.sid());
            }
        }

        None => {
            debug!("ROOMS: no socketid in room {}.", room_id);
        }
    }
}

pub fn get_sockets_for_room(room_id: &str) -> Option<ReadGuard<String, Vec<ChannelPair>>> {
    ROOMS.get(room_id)
}

pub fn get_sockets_number_of_room(room_id: &str) -> usize {
    match ROOMS.get(room_id) {
        Some(channels) => {
            return channels.len();
        }
        None => ()
    }

    0
}