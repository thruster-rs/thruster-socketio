use hyper::Body;
use std::future::Future;
use std::pin::Pin;
use thruster::context::basic_hyper_context::{generate_context, BasicHyperContext as Ctx};
use thruster::context::hyper_request::HyperRequest;
use thruster::hyper_server::HyperServer;
use thruster::middleware::file::get_file;
use thruster::{async_middleware, m, middleware_fn, Context};
use thruster::{App, ThrusterServer};
use thruster::{MiddlewareNext, MiddlewareResult};

use dotenv::dotenv;
use env_logger;
use std::env;
use thruster_socketio::redis_pubsub::{connect_to_pubsub, RedisAdapter};
use thruster_socketio::{adapter, handle_io, socketio_handler, socketio_listener, SocketIO};
use tokio;

#[middleware_fn]
async fn noop(context: Ctx, _next: MiddlewareNext<Ctx>) -> MiddlewareResult<Ctx> {
    println!("nooping");

    Ok(context)
}

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
    println!("Requesting IO");
    handle_io(context, handle).await
}

#[middleware_fn]
async fn index(mut context: Ctx, _next: MiddlewareNext<Ctx>) -> MiddlewareResult<Ctx> {
    let content = get_file("socketio_middleware/examples/chat.html").unwrap();
    context.body = Body::from(content);
    Ok(context)
}

#[middleware_fn]
async fn cors(context: Ctx, next: MiddlewareNext<Ctx>) -> MiddlewareResult<Ctx> {
    let mut context = next(context).await?;

    context.set("Access-Control-Allow-Origin", "https://bridge.mydomain.com");
    context.set("Access-Control-Allow-Headers", "*");
    context.set(
        "Access-Control-Allow-Methods",
        "GET, POST, PUT, DELETE, OPTIONS",
    );
    context.set("Access-Control-Allow-Credentials", "true");

    Ok(context)
}

#[tokio::main]
async fn main() {
    let _ = env_logger::init();

    println!("Starting server at {:#?}", std::env::current_dir());

    dotenv().ok();

    let host = env::var("HOST").unwrap_or("0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or("4321".to_string());

    // tokio::spawn(async {
    //     connect_to_pubsub("redis://127.0.0.1", "socketio-example")
    //         .await
    //         .expect("Could not connect to redis :(");
    //     adapter(RedisAdapter {});
    // });

    let mut app = App::<HyperRequest, Ctx, ()>::create(generate_context, ())
        .middleware("/", m![cors])
        .get("/socket.io/*", m![io])
        .get("/", m![index])
        .options("/socket.io/*", m![noop]);

    app.connection_timeout = 30000;

    HyperServer::new(app)
        .with_upgrades(true)
        .build(&host, port.parse::<u16>().unwrap())
        .await;
}
