use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{ready, Context, Poll},
};

use pin_project::pin_project;
use tower::{Layer, Service};
use tracing::info;

pub trait SuccessChecker: Clone {
    type Request;
    type Response;

    fn should_monitor_response(&self, req: &Self::Request) -> bool;
    fn is_successful_response(&self, res: &Self::Response) -> bool;
}

#[derive(Debug)]
pub struct GrpcChecker<I, O>(PhantomData<(I, O)>);

impl<I, O> Clone for GrpcChecker<I, O> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl<I, O> Default for GrpcChecker<I, O> {
    fn default() -> Self {
        Self(PhantomData {})
    }
}

impl<I, O> GrpcChecker<I, O> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<I, O> SuccessChecker for GrpcChecker<I, O> {
    type Request = http::Request<I>;
    type Response = http::Response<O>;

    fn should_monitor_response(&self, req: &http::Request<I>) -> bool {
        info!("headers = {:?}", req.headers());
        matches!(
            req.headers().get("Content-Type").map(|h| h.to_str()),
            Some(Ok("application/grpc"))
        )
    }

    fn is_successful_response(&self, res: &http::Response<O>) -> bool {
        info!("headers = {:?}", res.headers());
        res.status().is_success()
            && res
                .headers()
                .get("grpc-status")
                .map(|grpc_status| grpc_status == "0")
                .unwrap_or(true)
    }
}

#[derive(Debug)]
pub struct HttpChecker<I, O>(PhantomData<(I, O)>);

impl<I, O> Clone for HttpChecker<I, O> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl<I, O> Default for HttpChecker<I, O> {
    fn default() -> Self {
        Self(PhantomData {})
    }
}

impl<I, O> HttpChecker<I, O> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<I, O> SuccessChecker for HttpChecker<I, O> {
    type Request = http::Request<I>;
    type Response = http::Response<O>;

    fn should_monitor_response(&self, req: &http::Request<I>) -> bool {
        info!("headers = {:?}", req.headers());
        true
    }

    fn is_successful_response(&self, res: &Self::Response) -> bool {
        info!("headers = {:?}", res.headers());
        res.status().is_success()
    }
}

#[derive(Debug, Clone, Default)]
pub struct RequestCounterLayer<C> {
    request_checker: C,
    counter_inner: Arc<Mutex<RequestCounterInner>>,
}

impl<I, O> RequestCounterLayer<HttpChecker<I, O>> {
    pub fn new_for_http() -> Self {
        RequestCounterLayer::new(HttpChecker::new())
    }
}

impl<I, O> RequestCounterLayer<GrpcChecker<I, O>> {
    pub fn new_for_grpc() -> Self {
        RequestCounterLayer::new(GrpcChecker::new())
    }
}

#[derive(Debug, Default)]
pub struct RequestCounterInner {
    counter: usize,
    counter_success: usize,
}

impl<C> RequestCounterLayer<C> {
    pub fn new(request_checker: C) -> Self {
        Self {
            request_checker,
            counter_inner: Default::default(),
        }
    }
}

impl<C, S> Layer<S> for RequestCounterLayer<C>
where
    C: Clone,
{
    type Service = RequestCounterService<C, S>;

    fn layer(&self, inner: S) -> Self::Service {
        Self::Service {
            req_res_checker: self.request_checker.clone(),
            counter_inner: self.counter_inner.clone(),
            inner,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RequestCounterService<C, S> {
    req_res_checker: C,
    counter_inner: Arc<Mutex<RequestCounterInner>>,
    inner: S,
}

impl<C, S, I, O> Service<I> for RequestCounterService<C, S>
where
    C: SuccessChecker<Request = I, Response = O>,
    S: Service<I, Response = O>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = RequestCounterFut<C, S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: I) -> Self::Future {
        if self.req_res_checker.should_monitor_response(&req) {
            RequestCounterFut::Monitored {
                req_res_checker: self.req_res_checker.clone(),
                counter_inner: self.counter_inner.clone(),
                fut: self.inner.call(req),
            }
        } else {
            RequestCounterFut::Other(self.inner.call(req))
        }
    }
}

#[pin_project(project = RequestCounterFutProj)]
pub enum RequestCounterFut<C, F> {
    Monitored {
        req_res_checker: C,
        counter_inner: Arc<Mutex<RequestCounterInner>>,
        #[pin]
        fut: F,
    },
    Other(#[pin] F),
}

impl<C, F, O, E> Future for RequestCounterFut<C, F>
where
    C: SuccessChecker<Response = O>,
    F: Future<Output = Result<O, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this {
            RequestCounterFutProj::Monitored {
                req_res_checker,
                counter_inner,
                fut,
            } => {
                let rdy = ready!(fut.poll(cx));
                let mut counters = counter_inner.lock().unwrap();
                counters.counter += 1;

                if let Ok(resp) = rdy.as_ref() {
                    if req_res_checker.is_successful_response(resp) {
                        counters.counter_success += 1;
                    }
                }

                let requests_count = counters.counter;
                let requests_success_count = counters.counter_success;
                info!("{requests_success_count}/{requests_count} requests have been successful");
                Poll::Ready(rdy)
            }
            RequestCounterFutProj::Other(f) => f.poll(cx),
        }
    }
}
