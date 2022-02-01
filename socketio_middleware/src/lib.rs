#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
extern crate log;

pub use thruster_socketio_proc::*;

pub mod redis_pubsub;
mod rooms;
mod sid;
mod socketio;
mod socketio_context;
mod socketio_message;
// mod socketio_parser;
mod socketio_upgrade;

pub use rooms::{get_sockets_for_room, get_sockets_number_for_room, print_sockets_for_room, get_rooms_number};
pub use socketio::{adapter, broadcast, SocketIOAdapter, SocketIOSocket as SocketIO};
pub use socketio_context::SocketIOContext;
pub use socketio_upgrade::handle_io;
