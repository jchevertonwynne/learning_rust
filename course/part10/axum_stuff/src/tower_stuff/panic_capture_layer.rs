use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use pin_project::pin_project;
use tower::{Layer, Service};

#[derive(Debug, Default, Clone)]
pub struct PanicCaptureLayer;

impl<S> Layer<S> for PanicCaptureLayer {
    type Service = PanicCaptureService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        PanicCaptureService { inner }
    }
}

#[derive(Debug, Clone)]
pub struct PanicCaptureService<S> {
    inner: S,
}

impl<S, I> Service<I> for PanicCaptureService<S>
where
    S: Service<I, Response = Response, Error = Infallible>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = PanicCaptureFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: I) -> Self::Future {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.inner.call(req))) {
            Ok(fut) => PanicCaptureFut::Fut(fut),
            Err(_) => PanicCaptureFut::Panicked,
        }
    }
}

#[pin_project(project = PanicCaptureFutProjection)]
pub enum PanicCaptureFut<F> {
    Panicked,
    Fut(#[pin] F),
}

impl<F> Future for PanicCaptureFut<F>
where
    F: Future<Output = Result<Response, Infallible>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            PanicCaptureFutProjection::Panicked => Poll::Ready(Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                "PanicCaptureService::call panicked",
            )
                .into_response())),
            PanicCaptureFutProjection::Fut(fut) => {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fut.poll(cx))) {
                    Ok(polled) => polled,
                    Err(_) => Poll::Ready(Ok((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "PanicCaptureFut::poll panicked",
                    )
                        .into_response())),
                }
            }
        }
    }
}
