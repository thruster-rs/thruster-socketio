use base64;
use crypto;
use crypto::digest::Digest;
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use std::boxed::Box;
use std::future::Future;
use std::pin::Pin;
use thruster::context::hyper_request::HyperRequest;
use thruster::context::basic_hyper_context::{BasicHyperContext as Ctx};
use thruster::{Context, MiddlewareResult};
use tokio;
use tokio_tungstenite::tungstenite::Message;

use crate::socketio::{
    SocketIOSocket, SocketIOWrapper as SocketIO, SOCKETIO_EVENT_OPEN,
    SOCKETIO_PING,
    WSSocketMessage,
    InternalMessage,
};
use crate::sid::generate_sid;

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

pub async fn handle_io(
    mut context: Ctx,
    handler: fn(SocketIOSocket) -> Pin<Box<dyn Future<Output = Result<SocketIOSocket, ()>> + Send>>,
) -> MiddlewareResult<Ctx> {
    let request = context.hyper_request.unwrap().request;

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

        context = Ctx::default();
        context.status(101);
        context.set("upgrade", "websocket");
        context.set("Sec-WebSocket-Accept", &accept_value);
        context.set("connection", "Upgrade");

        let sid = generate_sid();
        let body = serde_json::to_string(&HandshakeResponseData {
            sid: sid.clone(), // must be unique
            upgrades: vec!["websocket".to_string()],
            ping_interval: 25000,
            ping_timeout: 5000,
        })
        .unwrap();

        let encoded_opener = format!("0{}", body);

        // Spawn a separate future to handle this connection
        tokio::spawn(async move {
            let upgraded_req = request.into_body().on_upgrade().await.unwrap();

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
            let _ = ws_sender
                .send(Message::Text(SOCKETIO_EVENT_OPEN.to_string()))
                .await;
            let mut msg_fut = ws_receiver.next();
            let socket_wrapper = SocketIO::new(sid.clone(), ws_sender);
            let sender = socket_wrapper.sender();
            let receiver = socket_wrapper.receiver();

            tokio::spawn(async move {
                socket_wrapper.listen().await;
            });

            let socket = SocketIOSocket::new(
                sid.clone(),
                sender.clone(),
                receiver.clone(),
            );
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
                                let _ = sender.send(InternalMessage::WS(WSSocketMessage::RawMessage(val.to_string())));
                            }
                        };
                    }
                    Some(Ok(Message::Binary(_ws_payload))) => {
                        // No idea what to do here, pass to the handler I guess?
                    }
                    Some(Ok(Message::Ping(_))) => {
                        let _ = sender.send(InternalMessage::WS(WSSocketMessage::WsPing));
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // This is a server side application, and should not be pinging
                        break;
                    }
                    Some(Err(_e)) => {
                        break;
                    }
                    Some(Ok(Message::Close(_e))) => {
                        break;
                    },
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
        let body = serde_json::to_string(&HandshakeResponseData {
            sid: generate_sid(), // must be unique
            upgrades: vec!["websocket".to_string()],
            ping_interval: 25000,
            ping_timeout: 5000,
        })
        .unwrap();

        let encoded = format!("{}:0'{}'2:40", body.len(), body);

        context = Ctx::new(HyperRequest::default());
        context.set_body(encoded.as_bytes().to_vec());

        Ok(context)
    }
}
