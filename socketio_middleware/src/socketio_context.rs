use hyper::body::Body;
use hyper::Request;
use thruster::context::basic_hyper_context::BasicHyperContext;
use thruster::context::typed_hyper_context::TypedHyperContext;

pub trait SocketIOContext {
    /// Returns the associated hyper request for this context.
    fn into_request(self) -> Request<Body>;

    /// Sets the status of the context.
    fn status(&mut self, code: u32);
}

impl SocketIOContext for BasicHyperContext {
    fn into_request(self) -> Request<Body> {
        self.hyper_request.unwrap().request
    }

    fn status(&mut self, code: u32) {
        self.status(code);
    }
}

impl<T: 'static + Send + Sync> SocketIOContext for TypedHyperContext<T> {
    fn into_request(self) -> Request<Body> {
        self.hyper_request.unwrap().request
    }

    fn status(&mut self, code: u32) {
        self.status(code);
    }
}
