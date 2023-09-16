use pin_project::pin_project;
use std::{
    convert::Infallible,
    future::Future,
    pin::{pin, Pin},
    task::{ready, Context, Poll},
    time::Duration,
};
use tokio::time::Instant;
use tower::Service;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Hello, world!");

    let mut u = RaceService {
        inner1: UnitService,
        inner2: AddOneAndDoubleResponseService { inner: UnitService },
    };

    for _ in 0..100 {
        let resp = u.call(5).await;

        println!("resp = {resp:?}");
    }

    Ok(())
}

struct UnitService;

impl Service<usize> for UnitService {
    type Response = usize;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<usize, Infallible>>>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: usize) -> Self::Future {
        Box::pin(async move {
            let sleep = 10 + ((rand::random::<u8>() as u64) % 10);
            tokio::time::sleep(Duration::from_millis(sleep)).await;
            Ok(req)
        })
    }
}

struct AddOneAndDoubleResponseService<S> {
    inner: S,
}

impl<S> Service<usize> for AddOneAndDoubleResponseService<S>
where
    S: Service<usize, Response = usize>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = AddOneAndDoubleResponseFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: usize) -> Self::Future {
        AddOneAndDoubleResponseFut {
            fut: self.inner.call(req + 1),
        }
    }
}

#[pin_project]
struct AddOneAndDoubleResponseFut<F> {
    #[pin]
    fut: F,
}

impl<F, E> Future for AddOneAndDoubleResponseFut<F>
where
    F: Future<Output = Result<usize, E>>,
{
    type Output = Result<usize, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let response = ready!(this.fut.poll(cx));
        Poll::Ready(response.map(|r| r * 2))
    }
}

struct TimerService<S> {
    inner: S,
}

impl<S, Req> Service<Req> for TimerService<S>
where
    S: Service<Req>,
{
    type Response = (S::Response, Duration);
    type Error = (S::Error, Duration);
    type Future = TimerFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        _ = ready!(self.inner.poll_ready(cx));
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Req) -> Self::Future {
        TimerFut {
            inner: self.inner.call(req),
            start: None,
        }
    }
}

#[pin_project]
struct TimerFut<F> {
    #[pin]
    inner: F,
    start: Option<Instant>,
}

impl<F, T, E> Future for TimerFut<F>
where
    F: Future<Output = Result<T, E>>,
{
    type Output = Result<(T, Duration), (E, Duration)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        if this.start.is_none() {
            _ = this.start.insert(Instant::now());
        }

        let resp = ready!(this.inner.poll(cx));

        let end = this
            .start
            .take()
            .expect("this should always be set")
            .elapsed();

        Poll::Ready(resp.map(|r| (r, end)).map_err(|e| (e, end)))
    }
}

struct RaceService<S1, S2> {
    inner1: S1,
    inner2: S2,
}

impl<S1, S2, Req, Res, Err> Service<Req> for RaceService<S1, S2>
where
    S1: Service<Req, Response = Res, Error = Err>,
    S2: Service<Req, Response = Res, Error = Err>,
    Req: Clone,
{
    type Response = Res;
    type Error = Err;
    type Future = RaceFut<S1::Future, S2::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        _ = ready!(self.inner1.poll_ready(cx));
        _ = ready!(self.inner2.poll_ready(cx));
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Req) -> Self::Future {
        RaceFut {
            inner1: self.inner1.call(req.clone()),
            inner2: self.inner2.call(req),
        }
    }
}

#[pin_project]
struct RaceFut<F1, F2> {
    #[pin]
    inner1: F1,
    #[pin]
    inner2: F2,
}

impl<F1, F2, O> Future for RaceFut<F1, F2>
where
    F1: Future<Output = O>,
    F2: Future<Output = O>,
{
    type Output = O;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        if let Poll::Ready(r1) = this.inner1.poll(cx) {
            return Poll::Ready(r1);
        }

        if let Poll::Ready(r2) = this.inner2.poll(cx) {
            return Poll::Ready(r2);
        }

        Poll::Pending
    }
}
