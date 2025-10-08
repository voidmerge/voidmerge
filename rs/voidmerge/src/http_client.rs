//! VoidMerge http client.

use crate::*;
use bytes::Bytes;

/// Configuration for an [HttpClient] instance.
#[derive(Default)]
#[non_exhaustive]
pub struct HttpClientConfig {}

/// VoidMerge http client.
pub struct HttpClient {
    client: reqwest::Client,
}

impl HttpClient {
    /// Construct a new [HttpClient].
    pub fn new(config: HttpClientConfig) -> Self {
        let _config = config;
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Execute a health check at the given url.
    pub async fn health(&self, url: &str) -> Result<()> {
        let mut url: reqwest::Url =
            url.parse().map_err(std::io::Error::other)?;
        url.set_path("");
        let res = self
            .client
            .get(url)
            .send()
            .await
            .map_err(std::io::Error::other)?;
        if res.error_for_status_ref().is_err() {
            return Err(std::io::Error::other(
                res.text().await.map_err(std::io::Error::other)?,
            ));
        }
        Ok(())
    }

    /// Setup a context on a VoidMerge server.
    pub async fn ctx_setup(
        &self,
        url: &str,
        token: &str,
        ctx_setup: crate::server::CtxSetup,
    ) -> Result<()> {
        let mut url: reqwest::Url =
            url.parse().map_err(std::io::Error::other)?;
        url.set_path("ctx-setup");
        let token = format!("Bearer {}", &token);
        let res = self
            .client
            .put(url)
            .header("Authorization", token)
            .body(Bytes::from_encode(&ctx_setup)?)
            .send()
            .await
            .map_err(std::io::Error::other)?;
        if res.error_for_status_ref().is_err() {
            return Err(std::io::Error::other(
                res.text().await.map_err(std::io::Error::other)?,
            ));
        }
        Ok(())
    }

    /// Configure a context on a VoidMerge server.
    pub async fn ctx_config(
        &self,
        url: &str,
        token: &str,
        ctx_config: crate::server::CtxConfig,
    ) -> Result<()> {
        let mut url: reqwest::Url =
            url.parse().map_err(std::io::Error::other)?;
        url.set_path(&format!("{}/_vm_/config", &ctx_config.ctx));
        let token = format!("Bearer {}", &token);
        let res = self
            .client
            .put(url)
            .header("Authorization", token)
            .body(Bytes::from_encode(&ctx_config)?)
            .send()
            .await
            .map_err(std::io::Error::other)?;
        if res.error_for_status_ref().is_err() {
            return Err(std::io::Error::other(
                res.text().await.map_err(std::io::Error::other)?,
            ));
        }
        Ok(())
    }
}
