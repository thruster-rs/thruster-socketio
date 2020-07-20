use trezm_redis::RedisResult;
use futures_util::StreamExt;
use trezm_redis::AsyncCommands;
use tokio;
use crossbeam::channel::{unbounded, Sender};
use std::sync::RwLock;
use log::{info, error};

use crate::socketio_message::{SocketIOMessage};
use crate::socketio::{SocketIOAdapter, InternalMessage};
use crate::rooms::get_sockets_for_room;
use crate::sid::generate_sid;

lazy_static! {
  static ref CHANNEL: RwLock<Vec<Sender<SocketIOToRedisMessage>>> = RwLock::new(Vec::new());
}

#[derive(Clone)]
pub struct RedisAdapter {

}

impl SocketIOAdapter for RedisAdapter {
  fn incoming(&self, room_id: &str, message: &SocketIOMessage) {
    // Here we need to relay the message to the redis pubsub
    // This is client -> us -> redis
    send_message(room_id, message.clone())
  }

  fn outgoing(&self, _room_id: &str, _message: &SocketIOMessage) {
    // Here we need to forward the message
    // This is redis -> us -> client
    // Automagically handled by the listener
  }
}

struct SocketIOToRedisMessage {
  room_id: String,
  socket_io_message: SocketIOMessage,
}

#[derive(Serialize, Deserialize, Debug)]
struct RedisMessage {
  channel: String,
  room_id: String,
  event: String,
  message: String,
  sending_id: String,
}

pub fn send_message(room_id: &str, message: SocketIOMessage) {
  for sender in &*CHANNEL.read().unwrap() {
    let socket_io_to_redis_message = match message {
      SocketIOMessage::Message(ref event, ref message) => Some(SocketIOToRedisMessage {
        room_id: room_id.to_owned(),
        socket_io_message: SocketIOMessage::Message(event.clone(), message.clone()),
      }),
      SocketIOMessage::SendMessage(ref event, ref message) => Some(SocketIOToRedisMessage {
        room_id: room_id.to_owned(),
        socket_io_message: SocketIOMessage::SendMessage(event.clone(), message.clone()),
      }),
      SocketIOMessage::Join(_) => None,
      SocketIOMessage::AddListener(_, _) => None,
      _ => {
        error!("Received a message that was not RawMessage, Message, or SendMessage: {}", message);
        None
      }
    };

    if let Some(val) = socket_io_to_redis_message {
      let _ = sender.send(val);
    }
  }
}

pub async fn connect_to_pubsub(redis_host: &str, channel_name: &str) -> RedisResult<()> {
  let redis_host = redis_host.to_string();
  let channel_name = channel_name.to_string();

  let client = trezm_redis::Client::open(redis_host).unwrap();
  let mut publish_conn = client.get_async_connection().await?;

  let (sender, receiver) = unbounded();

  CHANNEL.write().unwrap().push(sender);

  let channel_name = channel_name.to_string();
  let channel_name_outgoing = channel_name.clone();
  let channel_name_incoming = channel_name.clone();
  let sending_id = generate_sid();
  let sending_id_outgoing = sending_id.clone();
  let sending_id_incoming = sending_id.clone();

  // Handle pubbing local requests into redis
  tokio::spawn(async move {
    while let Ok(val) = receiver.recv() {
      info!("local -> redis: {} {}", val.room_id, val.socket_io_message);

      match val.socket_io_message {
        SocketIOMessage::SendMessage(event, message) => {
          let _ = publish_conn.publish::<'_, _, _, String>(channel_name_outgoing.clone(), serde_json::to_string(&RedisMessage {
            channel: channel_name_outgoing.clone(),
            room_id: val.room_id.clone(),
            event,
            message,
            sending_id: sending_id_outgoing.clone(),
          }).unwrap()).await;
        },
        SocketIOMessage::Message(event, message) => {
          let _ = publish_conn.publish::<'_, _, _, String>(channel_name_outgoing.clone(), serde_json::to_string(&RedisMessage {
            channel: channel_name_outgoing.clone(),
            room_id: val.room_id.clone(),
            event,
            message,
            sending_id: sending_id_outgoing.clone(),
          }).unwrap()).await;
        },
        _ => ()
      }
    }
  });

  // Handle subbing local requests from redis
  tokio::spawn(async move {
    let mut pubsub_conn = client.get_async_connection().await.unwrap().into_pubsub();

    pubsub_conn.subscribe(channel_name_incoming).await.unwrap();
    let mut pubsub_stream = pubsub_conn.on_message();

    while let Some(msg) = pubsub_stream.next().await {
      let message: RedisMessage = serde_json::from_str(&msg.get_payload::<String>().unwrap()).unwrap();

      info!("redis -> local: {} {} {}", message.room_id, message.event, message.message);

      if message.sending_id != sending_id_incoming {
        match get_sockets_for_room(&message.room_id) {
          Some(sockets) => {
            for socket in &*sockets {
              socket.send(InternalMessage::IO(SocketIOMessage::SendMessage(
                  message.event.to_string(),
                  message.message.to_string(),
              )));
            }
          },
          None => ()
        };
      }
    }
  });

  Ok(())
}
