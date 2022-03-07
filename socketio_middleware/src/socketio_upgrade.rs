use base64;
use crypto;
use crypto::digest::Digest;
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use std::boxed::Box;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use thruster::{Context, MiddlewareResult};
use tokio;
use tokio::time::{self, Duration};
use tokio_tungstenite::tungstenite::Message;

use crate::sid::generate_sid;
use crate::socketio::{
    InternalMessage, SocketIOSocket, SocketIOWrapper as SocketIO, WSSocketMessage,
    SOCKETIO_EVENT_OPEN, SOCKETIO_PING,
};
use crate::socketio_context::SocketIOContext;

const WEBSOCKET_SEC: &'static str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HandshakeResponseData {
    sid: String,
    upgrades: Vec<String>,
    ping_interval: usize,
    ping_timeout: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HandshakeResponse {
    r#type: String,
    data: HandshakeResponseData,
}

enum AllowedVersions {
    V3,
    V4,
}

/// Handles any incoming socket.io requests for a particular context by using the passed in handler.
///
/// Defaults to a maximum message capacity of 16. If there are more connections, then messages can
/// (and will!) be dropped.
pub async fn handle_io<T: Context + SocketIOContext + Default>(
    context: T,
    handler: fn(SocketIOSocket) -> Pin<Box<dyn Future<Output = Result<SocketIOSocket, ()>> + Send>>,
) -> MiddlewareResult<T> {
    handle_io_with_capacity(context, handler, 16).await
}

/// Handles any incoming socket.io requests for a particular context by using the passed in handler.
pub async fn handle_io_with_capacity<T: Context + SocketIOContext + Default>(
    mut context: T,
    handler: fn(SocketIOSocket) -> Pin<Box<dyn Future<Output = Result<SocketIOSocket, ()>> + Send>>,
    message_capacity: usize,
) -> MiddlewareResult<T> {
    let param_map = match context.route().split("?").collect::<Vec<&str>>().get(1) {
        Some(val) => {
            let mut map = HashMap::new();

            for el in val.split("&") {
                let mut split = el.split("=");

                map.insert(
                    split.next().unwrap_or_else(|| ""),
                    split.next().unwrap_or_else(|| ""),
                );
            }

            map
        }
        None => HashMap::new(),
    };

    let version = match param_map.get("EIO") {
        Some(&"4") => AllowedVersions::V4,
        _ => AllowedVersions::V3,
    };

    let mut request = context.into_request();

    // Theoretically should check this and the transport query param
    if request.headers().contains_key(hyper::header::UPGRADE) {
        let request_accept_key = request
            .headers()
            .get("Sec-WebSocket-Key")
            .unwrap()
            .to_str()
            .unwrap();
        let mut hasher = crypto::sha1::Sha1::new();
        hasher.input_str(&format!("{}{}", request_accept_key, WEBSOCKET_SEC));

        let mut accept_buffer = vec![0; hasher.output_bits() / 8];
        hasher.result(&mut accept_buffer);
        let accept_value = base64::encode(&accept_buffer);

        context = T::default();
        context.status(101);
        context.set("upgrade", "websocket");
        context.set("Sec-WebSocket-Accept", &accept_value);
        context.set("connection", "Upgrade");

        let sid = generate_sid();
        let body = serde_json::to_string(&HandshakeResponseData {
            sid: sid.clone(), // must be unique
            upgrades: vec!["websocket".to_string()],
            ping_interval: 25000,
            ping_timeout: 20000,
        })
        .unwrap();

        let encoded_opener = format!("0{}", body);

        // Spawn a separate future to handle this connection
        tokio::spawn(async move {
            let upgraded_req = hyper::upgrade::on(&mut request)
                .await
                .expect("Could not upgrade request to websocket");

            let ws_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(
                upgraded_req,
                tokio_tungstenite::tungstenite::protocol::Role::Server,
                None,
            )
            .await;
            let (mut ws_sender, mut ws_receiver) = ws_stream.split();

            // TODO(trezm): Handle errors here
            let _ = ws_sender.send(Message::Text(encoded_opener)).await;
            // TODO(trezm): Handle errors here

            match version {
                AllowedVersions::V3 => {
                    let _ = ws_sender
                        .send(Message::Text(SOCKETIO_EVENT_OPEN.to_string()))
                        .await;
                }
                AllowedVersions::V4 => {
                    let _ = ws_sender
                        .send(Message::Text(format!(
                            "{}{{\"sid\":\"{}\"}}",
                            SOCKETIO_EVENT_OPEN,
                            sid.clone()
                        )))
                        .await;
                }
            }

            let mut msg_fut = ws_receiver.next();
            let socket_wrapper = SocketIO::new(sid.clone(), ws_sender, message_capacity);
            let sender = socket_wrapper.sender();

            tokio::spawn(async move {
                socket_wrapper.listen().await;
            });

            // Keepalive in v4 is the server's responsibility.
            match version {
                AllowedVersions::V4 => {
                    let keepalive_sender = sender.clone();
                    tokio::spawn(async move {
                        let mut interval = time::interval(Duration::from_millis(25000));

                        loop {
                            interval.tick().await;

                            let res =
                                keepalive_sender.send(InternalMessage::WS(WSSocketMessage::Pong));

                            if res.is_err() {
                                break;
                            }
                        }
                    });
                }
                _ => (),
            };

            let socket = SocketIOSocket::new(sid.clone(), sender.clone());
            let _ = (handler)(socket)
                .await
                .expect("The handler should return a socket");

            loop {
                let val = msg_fut.await;

                match val {
                    Some(Ok(Message::Text(ws_payload))) => {
                        // TODO(trezm): Handle errors here
                        let _ = match ws_payload.as_ref() {
                            SOCKETIO_PING => {
                                let _ = sender.send(InternalMessage::WS(WSSocketMessage::Ping));
                            }
                            val => {
                                let _ = sender.send(InternalMessage::WS(
                                    WSSocketMessage::RawMessage(val.to_string()),
                                ));
                            }
                        };
                    }
                    Some(Ok(Message::Binary(_ws_payload))) => {
                        // No idea what to do here, pass to the handler I guess?
                    }
                    Some(Ok(Message::Ping(_))) => {
                        let _ = sender.send(InternalMessage::WS(WSSocketMessage::WsPing));
                        break;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        let _ = sender.send(InternalMessage::WS(WSSocketMessage::WsPong));
                        break;
                    }
                    Some(Err(_e)) => {
                        break;
                    }
                    Some(Ok(Message::Close(_e))) => {
                        break;
                    }
                    None => {
                        break;
                    }
                }

                msg_fut = ws_receiver.next();
            }

            // Cleanup the socket
            let _ = sender.send(InternalMessage::WS(WSSocketMessage::Close));
        });

        Ok(context)
    } else {
        let polling_enabled = request
            .uri()
            .to_string()
            .split('?')
            .nth(1)
            .map(|query_string| {
                query_string.split('&').fold(HashMap::new(), |mut acc, x| {
                    let mut pieces = x.split('=');
                    acc.insert(
                        pieces.next().unwrap_or_default(),
                        pieces.next().unwrap_or_default(),
                    );

                    acc
                })
            })
            .unwrap_or_default()
            .get("transport")
            .map(|v| v.contains("polling"))
            .unwrap_or(false);

        context = T::default();
        if !polling_enabled {
            context.status(400);
            context.set_body(
                "Polling transport disabled, but no upgrade header for websocket."
                    .as_bytes()
                    .to_vec(),
            );

            Ok(context)
        } else {
            context.set_body(
                "Polling transport is not implemented yet."
                    .as_bytes()
                    .to_vec(),
            );
            context.status(400);

            Ok(context)
        }
    }
}
