use crate::socketio_message::SocketIOMessage;

pub fn parse_socketio_message(payload: &str) -> (String, String) {
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

pub fn parse_ws_message(payload: &str) -> SocketIOMessage {
    match &payload[0..2] {
        SOCKETIO_EVENT_MESSAGE => {
            if payload.len() > 0 {
                let (event, message) = parse_socketio_message(&payload);
                SocketIOMessage::Message(event, message)
            } else {
                panic!(
                    "Attempted to handle a message payload with zero length '{}'",
                    payload
                )
            }
        }
        SOCKETIO_EVENT_CLOSE => SocketIOMessage::Close,
        // Also handle Non-messages
        _ => panic!("Attempted to handle a non-message payload: '{}'", payload),
    }
}
