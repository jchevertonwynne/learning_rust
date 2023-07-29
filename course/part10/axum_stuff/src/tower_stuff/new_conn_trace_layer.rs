use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

use hyper::server::conn::AddrStream;
use pin_project::pin_project;
use tower::{Layer, Service};
use tracing::{info, info_span, Span};

#[derive(Debug, Default)]
pub struct NewConnTraceLayer {}

impl<S> Layer<S> for NewConnTraceLayer {
    type Service = NewConnTraceService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        NewConnTraceService { inner }
    }
}

pub struct NewConnTraceService<S> {
    inner: S,
}

impl<'a, S> Service<&'a AddrStream> for NewConnTraceService<S>
where
    S: Service<&'a AddrStream>,
{
    type Response = TracedService<S::Response>;
    type Error = S::Error;
    type Future = NewConnTraceFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        info!("SERVICE POLL: checking if ready to make a new connection");
        let poll = self.inner.poll_ready(cx);
        if poll.is_ready() {
            info!("SERVICE POLL: ready!");
        } else {
            info!("SERVICE POLL: waiting...");
        }
        poll
    }

    fn call(&mut self, req: &'a AddrStream) -> Self::Future {
        info!(
            "SERVICE CALL: creating a new connection to {addr}",
            addr = req.remote_addr()
        );
        let span = info_span!("connection", addr=?req.remote_addr());
        NewConnTraceFut {
            span,
            fut: self.inner.call(req),
        }
    }
}

#[pin_project]
pub struct NewConnTraceFut<F> {
    span: Span,
    #[pin]
    fut: F,
}

impl<F, A, B> Future for NewConnTraceFut<F>
where
    F: Future<Output = Result<A, B>>,
{
    type Output = Result<TracedService<A>, B>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _entered = this.span.enter();
        info!("SERVICE FUTURE: polling to create a new service...");
        let rdy = ready!(this.fut.poll(cx));
        info!("SERVICE FUTURE: created a new connection");
        Poll::Ready(rdy.map(|inner| TracedService {
            span: this.span.clone(),
            inner,
        }))
    }
}

pub struct TracedService<S> {
    span: Span,
    inner: S,
}

impl<S, I> Service<I> for TracedService<S>
where
    S: Service<I>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = TracedServiceFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let _entered = self.span.enter();
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: I) -> Self::Future {
        let _entered = self.span.enter();
        TracedServiceFut {
            span: self.span.clone(),
            fut: self.inner.call(req),
        }
    }
}

#[pin_project]
pub struct TracedServiceFut<F> {
    span: Span,
    #[pin]
    fut: F,
}

impl<F> Future for TracedServiceFut<F>
where
    F: Future,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _entered = this.span.enter();
        this.fut.poll(cx)
    }
}
