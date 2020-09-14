use crate::socketio::SocketIOHandler;
use std::fmt;

#[derive(Clone)]
pub enum SocketIOMessage {
    Message(String, String), // Event, Message
    SendMessage(String, String),
    Join(String),
    Leave(String),
    AddListener(String, SocketIOHandler),
    Close,
    Pong,
    WsPong,
}

impl fmt::Display for SocketIOMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocketIOMessage::Message(event, message) => {
                write!(f, "SocketIOMessage::Message({}, {})", event, message)
            }
            SocketIOMessage::SendMessage(event, message) => {
                write!(f, "SocketIOMessage::SendMessage({}, {})", event, message)
            }
            SocketIOMessage::Join(val) => write!(f, "SocketIOMessage::Join({})", val),
            SocketIOMessage::Leave(val) => write!(f, "SocketIOMessage::Leave({})", val),
            SocketIOMessage::AddListener(val, _handler) => write!(f, "AddListener({})", val),
            SocketIOMessage::Close => write!(f, "SocketIOMessage::Close"),
            SocketIOMessage::Pong => write!(f, "SocketIOMessage::Pong"),
            SocketIOMessage::WsPong => write!(f, "SocketIOMessage::WsPong"),
        }
    }
}
