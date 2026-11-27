//! Per-client request limits (#58).
//!
//! Every API request counts against its client's budget: one for reads (`GET`,
//! `HEAD`) and a smaller one for writes. A client over budget gets `429` with
//! `Retry-After`. Limits use GCRA (the `governor` crate), so a client can burst
//! briefly but not sustain more than its rate.
//!
//! **Which client.** Behind the web app's proxy every request comes from the
//! web container, so the proxy forwards the visitor's address in a header and
//! `SF_CLIENT_IP_HEADER` names it. Without that setting the TCP peer address
//! is used. A request that lacks the configured header is the web app's own
//! server-side rendering and isn't limited: the coordinator is reachable only
//! through the proxy, which always sets it.
//!
//! **Privacy.** Client addresses are never stored or logged. Each one is hashed
//! with a key drawn at startup, and only the hash keys the limiter, so the
//! in-memory table can't be mapped back to addresses, even across restarts.

use std::{
    hash::{BuildHasher, Hasher},
    net::SocketAddr,
    num::NonZeroU32,
    sync::Arc,
    time::Duration,
};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header::RETRY_AFTER, HeaderName, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use governor::{
    clock::{Clock, DefaultClock},
    DefaultKeyedRateLimiter, Quota, RateLimiter,
};
use serde_json::json;

/// Request budgets per client.
#[derive(Clone, Debug)]
pub struct RateLimits {
    /// Sustained `GET`/`HEAD` requests per minute. 0 turns read limits off.
    pub reads_per_minute: u32,
    /// Sustained writes (`POST`, `PATCH`, `PUT`, `DELETE`) per minute. 0 turns
    /// write limits off.
    pub writes_per_minute: u32,
    /// Header carrying the client's address, set by a trusted proxy. `None`
    /// keys clients by the TCP peer address.
    pub client_header: Option<HeaderName>,
}

impl Default for RateLimits {
    fn default() -> Self {
        Self {
            reads_per_minute: 600,
            writes_per_minute: 120,
            client_header: None,
        }
    }
}

pub struct Limiter {
    reads: Option<DefaultKeyedRateLimiter<u64>>,
    writes: Option<DefaultKeyedRateLimiter<u64>>,
    client_header: Option<HeaderName>,
    key: std::collections::hash_map::RandomState,
}

/// A quarter of a minute's budget may arrive at once.
fn quota(per_minute: u32) -> Option<Quota> {
    let rate = NonZeroU32::new(per_minute)?;
    let burst = NonZeroU32::new((per_minute / 4).max(1))?;
    Some(Quota::per_minute(rate).allow_burst(burst))
}

impl Limiter {
    pub fn new(limits: &RateLimits) -> Self {
        Self {
            reads: quota(limits.reads_per_minute).map(RateLimiter::keyed),
            writes: quota(limits.writes_per_minute).map(RateLimiter::keyed),
            client_header: limits.client_header.clone(),
            key: Default::default(),
        }
    }

    /// The hashed client key, or `None` for a request that isn't limited.
    fn client(&self, req: &Request) -> Option<u64> {
        let address = match &self.client_header {
            Some(name) => req
                .headers()
                .get(name)?
                .to_str()
                .ok()?
                .split(',')
                .next()?
                .trim()
                .to_owned(),
            None => req
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()?
                .0
                .ip()
                .to_string(),
        };
        let mut hasher = self.key.build_hasher();
        hasher.write(address.as_bytes());
        Some(hasher.finish())
    }

    /// `Err(wait)` when the client is over budget.
    fn check(&self, req: &Request) -> Result<(), Duration> {
        let path = req.uri().path();
        if path == "/healthz" || path == "/readyz" {
            return Ok(());
        }
        let limiter = if matches!(*req.method(), Method::GET | Method::HEAD) {
            &self.reads
        } else {
            &self.writes
        };
        let (Some(limiter), Some(client)) = (limiter, self.client(req)) else {
            return Ok(());
        };
        limiter
            .check_key(&client)
            .map_err(|not_until| not_until.wait_time_from(DefaultClock::default().now()))
    }

    /// Forget clients that are back to a full budget, so the table only holds
    /// recent ones.
    pub fn prune(&self) {
        for limiter in [&self.reads, &self.writes].into_iter().flatten() {
            limiter.retain_recent();
            limiter.shrink_to_fit();
        }
    }
}

/// Axum middleware: refuse a request over its client's budget with `429`.
pub async fn limit(State(limiter): State<Arc<Limiter>>, req: Request, next: Next) -> Response {
    match limiter.check(&req) {
        Ok(()) => next.run(req).await,
        Err(wait) => {
            let seconds = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
            let mut res = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({
                    "code": "rate_limited",
                    "error": format!("too many requests; try again in {seconds} s"),
                })),
            )
                .into_response();
            if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
                res.headers_mut().insert(RETRY_AFTER, value);
            }
            res
        }
    }
}

/// Prune the limiter's table every minute.
pub async fn run_pruner(limiter: Arc<Limiter>) {
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    loop {
        tick.tick().await;
        limiter.prune();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    fn req(method: Method, path: &str, client: Option<&str>) -> Request {
        let mut b = Request::builder().method(method).uri(path);
        if let Some(c) = client {
            b = b.header("x-forwarded-for", c);
        }
        b.body(Body::empty()).unwrap()
    }

    fn limiter(reads: u32, writes: u32) -> Limiter {
        Limiter::new(&RateLimits {
            reads_per_minute: reads,
            writes_per_minute: writes,
            client_header: Some(HeaderName::from_static("x-forwarded-for")),
        })
    }

    #[test]
    fn each_client_has_its_own_budget() {
        let l = limiter(8, 4); // bursts of 2 reads, 1 write
        let a = || req(Method::GET, "/v1/auctions", Some("203.0.113.1"));
        assert!(l.check(&a()).is_ok());
        assert!(l.check(&a()).is_ok());
        let wait = l.check(&a()).unwrap_err();
        assert!(
            wait > Duration::ZERO && wait <= Duration::from_secs(8),
            "{wait:?}"
        );
        // Another client is unaffected; the first proxy hop is the client.
        let b = req(Method::GET, "/v1/auctions", Some("198.51.100.7, 10.0.0.2"));
        assert!(l.check(&b).is_ok());
    }

    #[test]
    fn writes_have_a_separate_smaller_budget() {
        let l = limiter(600, 4);
        let post = || req(Method::POST, "/v1/batches/x/shares", Some("203.0.113.1"));
        assert!(l.check(&post()).is_ok());
        assert!(l.check(&post()).is_err());
        // Reads still go through.
        assert!(l
            .check(&req(Method::GET, "/v1/chain", Some("203.0.113.1")))
            .is_ok());
    }

    #[test]
    fn health_checks_and_internal_requests_are_not_limited() {
        let l = limiter(4, 4);
        for _ in 0..10 {
            assert!(l
                .check(&req(Method::GET, "/healthz", Some("203.0.113.1")))
                .is_ok());
            // No client header: the web app's own server-side request.
            assert!(l.check(&req(Method::GET, "/v1/auctions", None)).is_ok());
        }
    }

    #[test]
    fn zero_turns_a_limit_off() {
        let l = limiter(0, 0);
        for _ in 0..100 {
            assert!(l
                .check(&req(Method::POST, "/v1/auctions", Some("x")))
                .is_ok());
        }
    }
}
