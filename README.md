# Thruster-SocketIO

This is a socket.io binding library for [thruster](https://github.com/thruster-rs/thruster). It allows developers to use thruster as the http server with a layer of socket.io over it.

**Note: This library is still under development. As of now, only websockets are implemented as a transport, polling on the other hand is a work in progress.**

### Usage

*Note: Currently, this library only works with a hyper-based server of thruster.*

To hook up the middleware, simply do the following:

```rust
let mut app = App::<HyperRequest, Ctx, ()>::create(generate_context, ());
...
app.get("/socket.io/*", async_middleware!(Ctx, [io]));
```

You'll need the io middleware which looks like this
```rust
use thruster_socketio::{handle_io, socketio_handler, socketio_listener, SocketIO};

...

#[socketio_listener]
async fn handle_a_message(socket: SocketIO, value: String) -> Result<(), ()> {
    println!("Handling [message]: {}", value);

    for room in socket.rooms() {
        println!("sending to a room: {}", room);
        socket.emit_to(room, "chat message", &value).await;
    }

    Ok(())
}

#[socketio_listener]
async fn join_room(mut socket: SocketIO, value: String) -> Result<(), ()> {
    println!("{} joining \"{}\"", socket.id(), &value);
    socket.join(&value).await;

    Ok(())
}

#[socketio_handler]
async fn handle<'a>(mut socket: SocketIO) -> Result<SocketIO, ()> {
    socket.on("chat message", handle_a_message);
    socket.on("join room", join_room);

    Ok(socket)
}

#[middleware_fn]
pub async fn io(context: Ctx, _next: MiddlewareNext<Ctx>) -> MiddlewareResult<Ctx> {
    handle_io(context, handle).await
}
```

There are a few key pieces in the above code:
* `io` is simply a middleware wrapper around the handler
* `handle` and the `#[socketio_handler]` macro represent the entrypoint for a socket when it's picked up by thruster. This is where you should add any socket initialization socket (on a per connection basis) as well as any listeners that you might want to add to a given socket.
* `handle_a_message`, `join_room`, and `#[socketio_listener]` are listeners (and a macro) that are fired when certain events are received from a socket. This is likely where the bulk of your logic and processing will live.

### Multi-server

Currently, we support redis as an adapter for messages. The usage of this is fairly seemless, simply add a block like this to your initialization logic:

```rust
use thruster_socketio::adapter;
use thruster_socketio::redis_pubsub::{
    connect_to_pubsub,
    RedisAdapter
};

...

tokio::spawn(async {
    let _ = connect_to_pubsub("redis://127.0.0.1/", "socketio-example").await.expect("Could not connect to redis :(");
    adapter(RedisAdapter{});
});
```
