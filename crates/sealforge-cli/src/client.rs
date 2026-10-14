//! HTTP client for the coordinator. Signs requests with `SF-Sig`.

use anyhow::bail;
use ed25519_dalek::SigningKey;
use reqwest::Method;
use sealforge_api::{auth::sign_request, types::ApiErrorBody};
use serde::{de::DeserializeOwned, Serialize};

#[derive(Clone)]
pub struct Client {
    base: String,
    http: reqwest::Client,
    key: Option<SigningKey>,
}

impl Client {
    pub fn new(base: &str, key: Option<SigningKey>) -> Self {
        Self {
            base: base.trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
            key,
        }
    }

    pub async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> anyhow::Result<T> {
        let bytes = match body {
            Some(b) => serde_json::to_vec(b)?,
            None => Vec::new(),
        };
        let mut req = self
            .http
            .request(method.clone(), format!("{}{path}", self.base))
            .header("content-type", "application/json");
        if let Some(key) = &self.key {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;
            let nonce: [u8; 16] = rand::random();
            req = req.header(
                "authorization",
                sign_request(key, method.as_str(), path, ts, &nonce, &bytes),
            );
        }
        let res = req.body(bytes).send().await?;
        let status = res.status();
        let body = res.bytes().await?;
        if !status.is_success() {
            match serde_json::from_slice::<ApiErrorBody>(&body) {
                Ok(err) => bail!("{} ({}): {}", status.as_u16(), err.code, err.error),
                Err(_) => bail!("{}: {}", status.as_u16(), String::from_utf8_lossy(&body)),
            }
        }
        let value: serde_json::Value = if body.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&body)?
        };
        Ok(serde_json::from_value(value)?)
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        self.request(Method::GET, path, None::<&()>).await
    }

    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> anyhow::Result<T> {
        self.request(Method::POST, path, Some(body)).await
    }

    pub async fn post_empty<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        self.request(Method::POST, path, None::<&()>).await
    }
}
