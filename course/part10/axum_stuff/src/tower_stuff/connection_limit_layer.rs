use hyper::server::conn::AddrStream;
use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{ready, Context, Poll},
};

use pin_project::pin_project;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::PollSemaphore;
use tower::{Layer, Service};
use tracing::info;

pub struct ConnectionLimitLayer {
    max: usize,
    sema: PollSemaphore,
}

impl ConnectionLimitLayer {
    pub fn new(max: usize) -> Self {
        Self {
            max,
            sema: PollSemaphore::new(Arc::new(Semaphore::new(max))),
        }
    }
}

impl<S> Layer<S> for ConnectionLimitLayer {
    type Service = ConnectionLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ConnectionLimitService {
            inner,
            max: self.max,
            sema: self.sema.clone(),
            permit: None,
        }
    }
}

pub struct ConnectionLimitService<S> {
    inner: S,
    max: usize,
    sema: PollSemaphore,
    permit: Option<OwnedSemaphorePermit>,
}

impl<'a, S> Service<&'a AddrStream> for ConnectionLimitService<S>
where
    S: Service<&'a AddrStream>,
{
    type Response = ConnectionLimitedServiceWrapper<S::Response>;
    type Error = S::Error;
    type Future = ConnectionLimitFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // If we haven't already acquired a permit from the semaphore, try to
        // acquire one first.
        if self.permit.is_none() {
            self.permit = ready!(self.sema.poll_acquire(cx));

            debug_assert!(
                self.permit.is_some(),
                "ConcurrencyLimit semaphore is never closed, so `poll_acquire` \
                 should never fail",
            );
        }

        // Once we've acquired a permit (or if we already had one), poll the
        // inner service.
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: &'a AddrStream) -> Self::Future {
        let permit = self.permit.take();

        debug_assert!(
            permit.is_some(),
            "permit should be set by the time we hit ConnectionLimitService::call"
        );

        info!(
            addr = ?req.remote_addr(),
            "creating a new limited connection",
        );

        ConnectionLimitFut {
            fut: self.inner.call(req),
            addr: req.remote_addr(),
            max: self.max,
            permit,
        }
    }
}

#[pin_project]
pub struct ConnectionLimitFut<F> {
    #[pin]
    fut: F,
    addr: SocketAddr,
    max: usize,
    permit: Option<OwnedSemaphorePermit>
}

impl<F, T, E> Future for ConnectionLimitFut<F>
where
    F: Future<Output = Result<T, E>>,
{
    type Output = Result<ConnectionLimitedServiceWrapper<T>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let rdy = ready!(this.fut.poll(cx));
        Poll::Ready(rdy.map(|inner| ConnectionLimitedServiceWrapper {
            inner,
            addr: *this.addr,
            _permit: this.permit.take()
        }))
    }
}

pub struct ConnectionLimitedServiceWrapper<S> {
    inner: S,
    addr: SocketAddr,
    _permit: Option<OwnedSemaphorePermit>
}

impl<S, I> Service<I> for ConnectionLimitedServiceWrapper<S>
where
    S: Service<I>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: I) -> Self::Future {
        self.inner.call(req)
    }
}

impl<S> Drop for ConnectionLimitedServiceWrapper<S> {
    fn drop(&mut self) {
        info!(
            addr = ?self.addr,
            "dropping connection limited service",
        );
    }
}
