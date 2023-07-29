use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

use axum::response::{IntoResponse, Response};
use http::StatusCode;
use pin_project::pin_project;
use tower::{Layer, Service};

#[derive(Debug, Clone, Default)]
pub struct EveryOtherRequestLayer {
    switch: Arc<AtomicBool>,
}

impl<S> Layer<S> for EveryOtherRequestLayer {
    type Service = EveryOtherService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        EveryOtherService::new(inner, self.switch.clone())
    }
}

#[derive(Debug, Clone)]
pub struct EveryOtherService<S> {
    inner: S,
    switch: Arc<AtomicBool>,
}

impl<S> EveryOtherService<S> {
    fn new(inner: S, switch: Arc<AtomicBool>) -> EveryOtherService<S> {
        EveryOtherService { inner, switch }
    }
}

impl<S, I> Service<S> for EveryOtherService<I>
where
    I: Service<S, Response = Response>,
{
    type Response = I::Response;
    type Error = I::Error;
    type Future = EveryOtherFut<I::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: S) -> Self::Future {
        if self.switch.fetch_xor(true, Ordering::Relaxed) {
            EveryOtherFut::Failed
        } else {
            EveryOtherFut::Fut(self.inner.call(req))
        }
    }
}

#[pin_project(project = EveryOtherFutProjection)]
pub enum EveryOtherFut<F> {
    Failed,
    Fut(#[pin] F),
}

impl<F, E> Future for EveryOtherFut<F>
where
    F: Future<Output = Result<Response, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            EveryOtherFutProjection::Failed => {
                Poll::Ready(Ok(StatusCode::FORBIDDEN.into_response()))
            }
            EveryOtherFutProjection::Fut(f) => f.poll(cx),
        }
    }
}
