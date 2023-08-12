use std::{
    future::{ready, Future, Ready},
    pin::Pin,
    task::{ready, Context, Poll},
    time::Duration,
};

use pin_project::pin_project;
use tokio::time::Sleep;
use tower::{
    retry::{future::ResponseFuture, Policy, Retry, RetryLayer},
    Layer,
    Service,
};
use tracing::info;

pub struct BackoffLayer<P, B> {
    retry: RetryLayer<BackoffPolicy<P>>,
    backoff: B,
}

impl<P, B> BackoffLayer<P, B> {
    pub fn new(policy: P, backoff_strategy: B) -> Self {
        Self {
            retry: RetryLayer::new(BackoffPolicy::new(policy)),
            backoff: backoff_strategy,
        }
    }
}

impl<S, P, B> Layer<S> for BackoffLayer<P, B>
where
    P: Clone,
    B: Clone,
{
    type Service = BackoffService<P, B, S>;

    fn layer(&self, inner: S) -> Self::Service {
        BackoffService::new(
            self.retry
                .layer(BackoffInnerService::new(inner, self.backoff.clone())),
        )
    }
}

#[derive(Clone)]
pub struct BackoffService<P, B, Req> {
    inner: Retry<BackoffPolicy<P>, BackoffInnerService<Req, B>>,
}

impl<P, B, Req> BackoffService<P, B, Req> {
    fn new(inner: Retry<BackoffPolicy<P>, BackoffInnerService<Req, B>>) -> Self {
        Self { inner }
    }
}

impl<P, B, S, Req> Service<Req> for BackoffService<P, B, S>
where
    P: Policy<Req, S::Response, S::Error> + Clone,
    B: BackoffStrategy,
    S: Service<Req> + Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<BackoffPolicy<P>, BackoffInnerService<S, B>, Backoff<Req>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        self.inner.call(Backoff::new(req))
    }
}

#[derive(Debug, Clone)]
pub struct BackoffInnerService<S, B> {
    inner: S,
    backoff: B,
}

impl<S, B> BackoffInnerService<S, B> {
    fn new(inner: S, backoff: B) -> Self {
        Self { inner, backoff }
    }
}

impl<S, B, Req> Service<Backoff<Req>> for BackoffInnerService<S, B>
where
    S: Service<Req>,
    B: BackoffStrategy,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BackoffFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Backoff<Req>) -> Self::Future {
        let Backoff { calls, req } = req;
        let backoff = self.backoff.backoff_duration(calls);
        let is_first_call = calls == 0;
        if !is_first_call {
            info!("this call will backoff for {backoff:?}");
        }

        BackoffFut::new(
            is_first_call,
            tokio::time::sleep(backoff),
            self.inner.call(req),
        )
    }
}

#[pin_project]
pub struct BackoffFut<F> {
    slept: bool,
    #[pin]
    sleep: Sleep,
    #[pin]
    fut: F,
}

impl<F> BackoffFut<F> {
    fn new(slept: bool, sleep: Sleep, fut: F) -> Self {
        Self { slept, sleep, fut }
    }
}

impl<F> Future for BackoffFut<F>
where
    F: Future,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        if !*this.slept {
            ready!(this.sleep.poll(cx));
            info!("backoff complete, trying call...");
            *this.slept = true;
        }

        this.fut.poll(cx)
    }
}

#[derive(Debug, Clone)]
pub struct BackoffPolicy<P> {
    inner: P,
}

impl<P> BackoffPolicy<P> {
    fn new(inner: P) -> Self {
        Self { inner }
    }
}

impl<P, Req, Res, Err> Policy<Backoff<Req>, Res, Err> for BackoffPolicy<P>
where
    P: Policy<Req, Res, Err> + Clone,
{
    type Future = Ready<Self>;

    fn retry(&self, req: &Backoff<Req>, result: Result<&Res, &Err>) -> Option<Self::Future> {
        let Backoff { req, .. } = req;
        self.inner
            .retry(req, result)
            .map(|_| ready((*self).clone()))
    }

    fn clone_request(&self, req: &Backoff<Req>) -> Option<Backoff<Req>> {
        let Backoff { calls, req } = req;
        self.inner
            .clone_request(req)
            .map(|req| Backoff::new_with_calls(req, calls + 1))
    }
}

pub struct Backoff<R> {
    calls: u32,
    req: R,
}

impl<R> Backoff<R> {
    fn new(req: R) -> Self {
        Self { calls: 0, req }
    }
    fn new_with_calls(req: R, calls: u32) -> Self {
        Self { calls, req }
    }
}

trait BackoffStrategy: Clone {
    fn backoff_duration(&self, repeats: u32) -> Duration;
}

pub mod backoff_strategies {
    use crate::tower_stuff::backoff_layer::BackoffStrategy;
    use std::time::Duration;

    #[derive(Debug, Clone)]
    pub struct ExponentialBackoffStrategy;

    impl BackoffStrategy for ExponentialBackoffStrategy {
        fn backoff_duration(&self, repeats: u32) -> Duration {
            Duration::from_millis(1 << repeats)
        }
    }

    #[derive(Debug, Clone)]
    pub struct FibonacciBackoffStrategy;

    impl BackoffStrategy for FibonacciBackoffStrategy {
        fn backoff_duration(&self, repeats: u32) -> Duration {
            let mut a = 0;
            let mut b = 1;
            for _ in 0..repeats {
                let c = a + b;
                a = b;
                b = c;
            }
            Duration::from_millis(a)
        }
    }

    #[derive(Debug, Clone)]
    pub struct LinearBackoffStrategy {
        duration_multiple: Duration,
    }

    impl LinearBackoffStrategy {
        pub fn new(duration_multiple: Duration) -> Self {
            Self { duration_multiple }
        }
    }

    impl BackoffStrategy for LinearBackoffStrategy {
        fn backoff_duration(&self, repeats: u32) -> Duration {
            self.duration_multiple * repeats
        }
    }
}
