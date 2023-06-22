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

        PARENT_CTX_MAP.with::<_, Result<_, anyhow::Error>>(|parent_ctx_map| {
            let mut parent_ctx_map = parent_ctx_map.borrow_mut();
            parent_ctx_map.clear();

            let ctx = tracing::Span::current().context();

            opentelemetry::global::get_text_map_propagator(|propagator| {
                propagator.inject_context(&ctx, parent_ctx_map.deref_mut());
            });

            let hd = req.headers_mut();
            for (k, v) in parent_ctx_map.drain() {
                let k = k.parse::<HeaderName>()?;
                let v = v.parse::<HeaderValue>()?;
                hd.insert(k, v);
            }

            Ok(())
        })?;

        next.run(req, extensions).await
    }
}
