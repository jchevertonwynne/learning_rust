use async_trait::async_trait;
use fxhash::FxBuildHasher;
use http::{HeaderName, HeaderValue};
use reqwest::{Request, Response};
use reqwest_middleware::Next;
use std::{cell::RefCell, collections::HashMap, ops::DerefMut};
use task_local_extensions::Extensions;
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[derive(Debug, Default)]
pub struct JaegerContextPropagatorMiddleware {}

impl JaegerContextPropagatorMiddleware {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl reqwest_middleware::Middleware for JaegerContextPropagatorMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        extensions: &'_ mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        std::thread_local! {
            static PARENT_CTX_MAP: RefCell<HashMap<String, String, FxBuildHasher>> = RefCell::new(HashMap::with_hasher(FxBuildHasher::default()));
        }

        if let Err(err) = PARENT_CTX_MAP.with(|parent_ctx_map| {
            let mut parent_ctx_map = parent_ctx_map.borrow_mut();
            parent_ctx_map.clear();

            let ctx = tracing::Span::current().context();

            opentelemetry::global::get_text_map_propagator(|propagator| {
                propagator.inject_context(&ctx, parent_ctx_map.deref_mut());
            });

            let hd = req.headers_mut();
            for (k, v) in parent_ctx_map.drain() {
                let k = match k.parse::<HeaderName>() {
                    Ok(k) => k,
                    Err(err) => return Err(anyhow::Error::new(err)),
                };
                let v = match v.parse::<HeaderValue>() {
                    Ok(v) => v,
                    Err(err) => return Err(anyhow::Error::new(err)),
                };
                hd.insert(k, v);
            }

            Ok::<_, anyhow::Error>(())
        }) {
            return Err(reqwest_middleware::Error::Middleware(err));
        }

        next.run(req, extensions).await
    }
}
