use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

use hyper::server::conn::AddrStream;
use pin_project::pin_project;
use tower::{Layer, Service};
use tracing::{debug, info_span, Span};

#[derive(Debug)]
pub struct NewConnSpanMakeServiceLayer;

impl<S> Layer<S> for NewConnSpanMakeServiceLayer {
    type Service = NewConnSpanMakeService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        NewConnSpanMakeService { inner }
    }
}

pub struct NewConnSpanMakeService<S> {
    inner: S,
}

impl<'a, S> Service<&'a AddrStream> for NewConnSpanMakeService<S>
where
    S: Service<&'a AddrStream>,
{
    type Response = SpannedService<S::Response>;
    type Error = S::Error;
    type Future = NewConnSpanFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        debug!("SERVICE POLL: checking if ready to make a new connection");
        let poll = self.inner.poll_ready(cx);
        if poll.is_ready() {
            debug!("SERVICE POLL: ready!");
        } else {
            debug!("SERVICE POLL: waiting...");
        }
        poll
    }

    fn call(&mut self, req: &'a AddrStream) -> Self::Future {
        debug!(
            "SERVICE CALL: creating a new connection to {addr}",
            addr = req.remote_addr()
        );
        let span = info_span!("connection", addr=?req.remote_addr());
        NewConnSpanFut {
            span,
            fut: self.inner.call(req),
        }
    }
}

#[pin_project]
pub struct NewConnSpanFut<F> {
    span: Span,
    #[pin]
    fut: F,
}

impl<F, T, E> Future for NewConnSpanFut<F>
where
    F: Future<Output = Result<T, E>>,
{
    type Output = Result<SpannedService<T>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _entered = this.span.enter();
        debug!("SERVICE FUTURE: polling to create a new service...");
        let rdy = ready!(this.fut.poll(cx));
        debug!("SERVICE FUTURE: created a new connection");
        Poll::Ready(rdy.map(|inner| SpannedService {
            span: this.span.clone(),
            inner,
        }))
    }
}

pub struct SpannedService<S> {
    span: Span,
    inner: S,
}

impl<S, I> Service<I> for SpannedService<S>
where
    S: Service<I>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = SpannedServiceFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let _entered = self.span.enter();
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: I) -> Self::Future {
        let _entered = self.span.enter();
        SpannedServiceFut {
            span: self.span.clone(),
            fut: self.inner.call(req),
        }
    }
}

#[pin_project]
pub struct SpannedServiceFut<F> {
    span: Span,
    #[pin]
    fut: F,
}

impl<F> Future for SpannedServiceFut<F>
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
