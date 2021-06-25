// use crossbeam::channel::unbounded;
// use crossbeam::channel::{Receiver, Sender};
use tokio::sync::broadcast::channel as unbounded;
use tokio::sync::broadcast::{Receiver, Sender};

use futures::stream::FuturesUnordered;
use futures_util::sink::SinkExt;
use futures_util::stream::SplitSink;
use log::{info, debug};
use std::boxed::Box;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::RwLock;
use tokio_stream::StreamExt;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::rooms::{
    get_sockets_for_room, join_channel_to_room, remove_socket_from_room, ChannelPair,
};
use crate::socketio_message::SocketIOMessage;

pub type SocketIOHandler =
    fn(SocketIOSocket, String) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send>>;

pub const SOCKETIO_PING: &'static str = "2";
pub const SOCKETIO_PONG: &'static str = "3";
pub const SOCKETIO_EVENT_OPEN: &'static str = "40"; // Message, then open
pub const SOCKETIO_EVENT_MESSAGE: &'static str = "42"; // Message, then event

lazy_static! {
    static ref ADAPTER: RwLock<Option<Box<dyn SocketIOAdapter>>> = RwLock::new(None);
}

///
/// Broadcast a message to all clients connected to a room.
///
pub async fn broadcast(room_id: &str, event: &str, message: &str) {
    // Send out via adapter
    if let Some(adapter) = &*ADAPTER.read().unwrap() {
        adapter.incoming(
            room_id,
            &SocketIOMessage::SendMessage(event.to_string(), message.to_string()),
        );
    }

    match get_sockets_for_room(room_id) {
        Some(channels) => {
            for channel in &*channels {
                    channel.send(InternalMessage::IO(SocketIOMessage::SendMessage(
                        event.to_string(),
                        message.to_string(),
                    )));
                    debug!("Found socketid {} in room {}, send message = {}",channel.sid(), room_id, message);
            }
        }
        None => {
            debug!("Found no socketid in room {}, not send message = {}", room_id, message);
        },
    }
}

pub fn adapter(new_adapter: impl SocketIOAdapter + 'static) {
    let mut adapter = ADAPTER.write().unwrap();
    adapter.replace(Box::new(new_adapter));
}

pub fn parse_raw_message(payload: &str) -> (String, String) {
    let message = &payload[2..];
    let leading_bracket = message
        .find("[")
        .unwrap_or_else(|| panic!("Found a message with no leading bracket: '{}'", message));
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

pub trait SocketIOAdapter: Send + Sync {
    fn incoming(&self, room_id: &str, message: &SocketIOMessage);
    fn outgoing(&self, room_id: &str, message: &SocketIOMessage);
}

#[derive(Clone, Debug)]
pub enum InternalMessage {
    IO(SocketIOMessage),
    WS(WSSocketMessage),
}

#[derive(Clone, Debug)]
pub enum WSSocketMessage {
    RawMessage(String),
    Close,
    Ping,
    Pong,
    WsPing,
    WsPong,
}

pub struct SocketIOSocket {
    id: String,
    sender: Sender<InternalMessage>,
    rooms: Vec<String>,
}

impl Clone for SocketIOSocket {
    fn clone(&self) -> Self {
        SocketIOSocket {
            id: self.id.clone(),
            sender: self.sender.clone(),
            rooms: self.rooms.clone(),
        }
    }
}

impl SocketIOSocket {
    pub fn new(id: String, sender: Sender<InternalMessage>) -> Self {
        SocketIOSocket {
            id,
            sender,
            rooms: Vec::new(),
        }
    }
    ///
    /// id returns the id for this particular socket.
    ///
    pub fn id(&self) -> &str {
        &self.id
    }

    ///
    /// use_handler isn't implemented yet.
    ///
    pub fn use_handler(&self, _handler: SocketIOHandler) {
        unimplemented!("use_handler isn't implemented yet.")
    }

    ///
    /// on adds a listener for a particular event
    ///
    pub fn on(&mut self, event: &str, handler: SocketIOHandler) {
        let _ = self
            .sender
            .send(InternalMessage::IO(SocketIOMessage::AddListener(
                event.to_string(),
                handler,
            )));
    }

    ///
    /// join joins a socket into a room. This makes every message sent
    /// by that socket go to the room rather than globally.
    ///
    pub async fn join(&mut self, room_id: &str) {
        let _ = self.sender.send(InternalMessage::IO(SocketIOMessage::Join(
            room_id.to_string(),
        )));
    }

    ///
    /// leave removes a socket from a room. Note that you cannot remove
    /// a socket from its default room, i.e. its SID. This will result
    /// in a noop.
    ///
    pub async fn leave(&mut self, room_id: &str) {
        let _ = self.sender.send(InternalMessage::IO(SocketIOMessage::Leave(
            room_id.to_string(),
        )));
    }

    ///
    /// send sends a message to this socket
    ///
    pub async fn send(&self, event: &str, message: &str) {
        let _ = self
            .sender
            .send(InternalMessage::IO(SocketIOMessage::SendMessage(
                event.to_string(),
                message.to_string(),
            )));
    }

    ///
    /// emit_to sends a message to all sockets connected to the given
    /// room_id, including the sending socket.
    ///
    pub async fn emit_to(&self, room_id: &str, event: &str, message: &str) {
        // Send out via adapter
        if let Some(adapter) = &*ADAPTER.read().unwrap() {
            adapter.incoming(
                room_id,
                &SocketIOMessage::SendMessage(event.to_string(), message.to_string()),
            );
        }

        match get_sockets_for_room(room_id) {
            Some(channels) => {
                for channel in &*channels {
                    channel.send(InternalMessage::IO(SocketIOMessage::SendMessage(
                        event.to_string(),
                        message.to_string(),
                    )));
                }
            }
            None => (),
        }
    }

    ///
    /// broadcast_to sends a message to all the sockets connected to
    /// the given room_id, excluding the sending socket.
    ///
    pub async fn broadcast_to(&self, room_id: &str, event: &str, message: &str) {
        // Send out via adapter
        if let Some(adapter) = &*ADAPTER.read().unwrap() {
            adapter.incoming(
                room_id,
                &SocketIOMessage::SendMessage(event.to_string(), message.to_string()),
            );
        }

        match get_sockets_for_room(room_id) {
            Some(channels) => {
                for channel in &*channels {
                    if channel.sid() != &self.id {
                        channel.send(InternalMessage::IO(SocketIOMessage::SendMessage(
                            event.to_string(),
                            message.to_string(),
                        )));
                    }
                }
            }
            None => (),
        }
    }

    ///
    /// rooms returns all of the rooms this socket is currently in
    ///
    pub fn rooms(&self) -> &Vec<String> {
        &self.rooms
    }
}

impl fmt::Display for InternalMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InternalMessage::IO(v) => write!(f, "Message::IO({})", v),
            InternalMessage::WS(v) => write!(f, "Message::WS({})", v),
        }
    }
}

impl fmt::Display for WSSocketMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WSSocketMessage::RawMessage(val) => write!(f, "WSSocketMessage::RawMessage({})", val),
            WSSocketMessage::Ping => write!(f, "WSSocketMessage::Ping"),
            WSSocketMessage::Pong => write!(f, "WSSocketMessage::Pong"),
            WSSocketMessage::WsPing => write!(f, "WSSocketMessage::WsPing"),
            WSSocketMessage::WsPong => write!(f, "WSSocketMessage::WsPong"),
            WSSocketMessage::Close => write!(f, "WSSocketMessage::Close"),
        }
    }
}

pub struct SocketIOWrapper {
    sid: String,
    message_number: usize,
    socket: SplitSink<WebSocketStream<hyper::upgrade::Upgraded>, Message>,
    rooms: Vec<String>,
    event_handlers: HashMap<String, Vec<SocketIOHandler>>,
    sender: Sender<InternalMessage>,
    receiver: Receiver<InternalMessage>,
}

impl SocketIOWrapper {
    pub fn new(
        sid: String,
        socket: SplitSink<WebSocketStream<hyper::upgrade::Upgraded>, Message>,
    ) -> Self {
        let (sender, receiver) = unbounded(16);
        SocketIOWrapper {
            sid,
            message_number: 0,
            socket,
            rooms: Vec::new(),
            event_handlers: HashMap::new(),
            sender,
            receiver,
        }
    }

    pub async fn close(mut self) {
        let _res = self.socket.close().await;
    }

    ///
    /// Handle an incoming payload. This parses the string into the correct parts and calls
    /// self.handler on them
    ///
    pub async fn handle(&mut self, payload: String) {
        if payload == SOCKETIO_PING {
            let _ = self.sender.send(InternalMessage::WS(WSSocketMessage::Pong));
            return;
        }

        if payload == SOCKETIO_PONG {
            // Probably should set a timer and send a ping, but really... eh?
            return;
        }

        match &payload[0..2] {
            "42" => {
                if payload.len() > 0 {
                    let (event, message) = parse_raw_message(&payload);

                    // Run handlers
                    match self.event_handlers.get(&event) {
                        Some(handlers) => {
                            // Run with each handler -- should they be async and waited for?
                            let unordered_future = FuturesUnordered::new();

                            for handler in handlers {
                                unordered_future.push((handler)(
                                    SocketIOSocket {
                                        id: self.sid.clone(),
                                        sender: self.sender.clone(),
                                        rooms: self.rooms.clone(),
                                    },
                                    message.clone(),
                                ));
                            }

                            // Dev note -- this must be spawned in a separate task, otherwise
                            // it can block the receive loop and queue up too many transactions.
                            tokio::spawn(async move {
                                let _ = unordered_future.collect::<Result<(), ()>>().await;
                            });
                        }
                        None => {
                            info!("No handler found for message: {:#?}", event);
                        } // Ignore
                    }
                }
            }
            "41" => {
                info!("{}: Socket closed...", self.sid);
            }
            "40" => {
                info!("{}: Socket opened...", self.sid);
            }
            _ => panic!("Attempted to handle a non-message payload: '{}'", payload),
        }
    }

    pub async fn listen(mut self) {
        while let Ok(val) = self.receiver.recv().await {
            match val {
                InternalMessage::IO(val) => {
                    match val {
                        SocketIOMessage::SendMessage(event, message) => {
                            self.message_number += 1;

                            let message = match &message[0..1] {
                                "{" | "[" => message,
                                _ => format!("\"{}\"", message),
                            };

                            // TODO(trezm): Payload needs to be quoted if just a string, not if it's json
                            let content = format!(
                                "{}{}[\"{}\",{}]",
                                SOCKETIO_EVENT_MESSAGE, self.message_number, event, message
                            );

                            let _ = self.socket.send(Message::Text(content)).await;
                        }

                        SocketIOMessage::Join(room_id) => {
                            // check if room_id exist. Don't use return because of the following process such as PING/PONG.
                            if false == self.rooms.contains(&room_id) {
                                self.rooms.push(room_id.to_string());
                                debug!("SocketIOMessage socketid {} joined into room {}. Rooms = {:?}, rooms len = {}", self.sid, room_id, self.rooms, self.rooms.len());                            

                                //Call rooms::join_channel_to_room
                                join_channel_to_room(
                                    &room_id,
                                    ChannelPair::new(&self.sid, self.sender()),
                                );
                            } else {
                                debug!("SocketIOMessage socketid {} doesn't join into room {}, this room exist.", self.sid, room_id);
                            }
                        }

                        SocketIOMessage::Leave(room_id) => {
                            let mut i = 0;
                            for room in &self.rooms {
                                if room == &room_id {
                                    self.rooms.remove(i);
                                    debug!("SocketIOMessage socketid {} leaved from room {}. Rooms = {:?}, rooms len = {}", self.sid, room_id, self.rooms, self.rooms.len());                            

                                    //Call rooms::remove_socket_from_room
                                    remove_socket_from_room(&room_id, &self.sid);
                                    break;
                                }

                                i = i + 1;
                            }
                        }

                        SocketIOMessage::AddListener(event, handler) => {
                            let mut existing_handlers = self
                                .event_handlers
                                .remove(&event)
                                .unwrap_or_else(|| Vec::new());

                            existing_handlers.push(handler);

                            self.event_handlers
                                .insert(event.to_string(), existing_handlers);
                        }
                        _ => (),
                    }
                }
                InternalMessage::WS(val) => match val {
                    WSSocketMessage::RawMessage(message) => self.handle(message).await,
                    WSSocketMessage::Ping => {
                        let _ = self
                            .socket
                            .send(Message::Text(SOCKETIO_PONG.to_string()))
                            .await;
                    }
                    WSSocketMessage::Pong => {
                        let _ = self
                            .socket
                            .send(Message::Text(SOCKETIO_PING.to_string()))
                            .await;
                    }
                    WSSocketMessage::WsPing => {
                        let _ = self.socket.send(Message::Pong([].to_vec())).await;
                    }
                    WSSocketMessage::WsPong => {
                        let _ = self.socket.send(Message::Ping([].to_vec())).await;
                    }

                    WSSocketMessage::Close => {
                        // remove the socket from all joined rooms
                        for room in &self.rooms {
                            remove_socket_from_room(&room, &self.sid);
                            debug!("SocketIOMessage socketid {} closed, leave from room {}", self.sid, room);                            
                        }

                        self.close().await;
                        return;
                    }
                }
            }
        }
    }

    pub fn sender(&self) -> Sender<InternalMessage> {
        self.sender.clone()
    }
}
