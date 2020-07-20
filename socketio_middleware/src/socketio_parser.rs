use crossbeam::channel::unbounded;
use crossbeam::channel::{Receiver, Sender};
use futures::stream::FuturesUnordered;
use futures_util::sink::SinkExt;
use futures_util::stream::SplitSink;
use std::boxed::Box;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::fmt;
use std::sync::RwLock;
use tokio::stream::StreamExt;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use log::info;

use crate::rooms::{get_sockets_for_room, join_channel_to_room, remove_socket_from_room, ChannelPair};
use crate::socketio_message::SocketIOMessage;

pub const SOCKETIO_PING: &'static str = "2";
pub const SOCKETIO_PONG: &'static str = "3";
pub const SOCKETIO_EVENT_OPEN: &'static str = "40"; // Message, then open
pub const SOCKETIO_EVENT_CLOSE: &'static str = "40"; // Message, then open
pub const SOCKETIO_EVENT_MESSAGE: &'static str = "42"; // Message, then event

pub fn parse_socketio_message(payload: &str) -> (String, String) {
    let message = &payload[2..];
    let leading_bracket = message.find("[").unwrap_or_else(|| {
        panic!("Found a message with no leading bracket: '{}'", message)
    });
    let event_split = message.find(",").unwrap_or_else(|| {
        panic!(
            "Received a message without a comma separator: '{}'",
            message
        )
    });

    let event = &message[leading_bracket + 2..event_split - 1];
    let mut content = &message[event_split + 1..message.len() - 1];

    if &content[0..1] == "\"" {
        content = &content[1..content.len() - 1];
    }

    (event.to_string(), content.to_string())
}

pub fn parse_ws_message(payload: &str) -> SocketIOMessage {
  match &payload[0..2] {
    SOCKETIO_EVENT_MESSAGE => {
        if payload.len() > 0 {
            let (event, message) = parse_socketio_message(&payload);
            SocketIOMessage::Message(event, message)
        } else {
            panic!("Attempted to handle a message payload with zero length '{}'", payload)
        }
    }
    SOCKETIO_EVENT_CLOSE => {
        SocketIOMessage::Close
    }
    // Also handle Non-messages
    _ => panic!("Attempted to handle a non-message payload: '{}'", payload)
  }
}
