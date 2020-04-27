use chashmap::{CHashMap, ReadGuard};

use crate::socketio::InternalMessage;
use crossbeam::channel::Sender;

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
}

pub fn remove_socket_from_room(room_id: &str, _sid: &str) {
    let mut connected_sockets = match ROOMS.remove(room_id) {
      Some(val) => val,
      None => Vec::new(),
    };

    for i in 0..connected_sockets.len() - 1 {
        let i = connected_sockets.len() - i;
        let socket = connected_sockets.get(i).unwrap();

        if socket.sid == room_id {
            connected_sockets.remove(i);
            break;
        }
    }

    ROOMS.insert(room_id.to_string(), connected_sockets);
}


pub fn get_sockets_for_room(room_id: &str) -> Option<ReadGuard<String, Vec<ChannelPair>>> {
    ROOMS.get(room_id)
}
