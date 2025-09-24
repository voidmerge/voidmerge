//! VoidMerge http client.

use crate::*;
use bytes::Bytes;
//use std::collections::HashMap;
//use types::*;

/// Configuration for an [HttpClient] instance.
#[derive(Default)]
#[non_exhaustive]
pub struct HttpClientConfig {}

/// VoidMerge http client.
pub struct HttpClient {
    client: reqwest::Client,
    /*
    token: Mutex<Option<Hash>>,
    auth_token: tokio::sync::Semaphore,
    sign: Arc<MultiSign>,
    app_auth_data: Mutex<HashMap<Hash, Value>>,
    */
}

impl HttpClient {
    /// Construct a new [HttpClient].
    pub fn new(config: HttpClientConfig/*, sign: Arc<MultiSign>*/) -> Self {
        let _config = config;
        Self {
            client: reqwest::Client::new(),
            /*
            token: Default::default(),
            auth_token: tokio::sync::Semaphore::new(1),
            sign,
            app_auth_data: Default::default(),
            */
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

    /// Setup/configure a context on a VoidMerge server.
    pub async fn ctx_setup(&self, url: &str, token: &str, ctx_setup: crate::server::CtxSetup) -> Result<()> {
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

    /*
    /// Set an explicit api token to use.
    pub fn set_api_token(&self, token: Hash) {
        *self.token.lock().unwrap() = Some(token);
    }

    /// Set any app-specific authentication data.
    pub fn set_app_auth_data(&self, ctx: Hash, app: Value) {
        self.app_auth_data.lock().unwrap().insert(ctx, app);
    }

    async fn get_auth_token(&self, mut url: reqwest::Url) -> Result<()> {
        // just a guard so we only do this once at a time
        let _g = self.auth_token.acquire().await.unwrap();
        url.set_path("/auth-chal-req");
        let req = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(std::io::Error::other)?;
        if req.error_for_status_ref().is_err() {
            return Err(std::io::Error::other(
                req.text().await.map_err(std::io::Error::other)?,
            ));
        }
        let req = req.bytes().await.map_err(std::io::Error::other)?;
        let req: AuthChalReq = decode(&req)?;

        let res = AuthChalRes {
            nonce_sig: self.sign.sign(&req.nonce),
            context_access: self
                .app_auth_data
                .lock()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        };

        let token = format!("Bearer {}", &req.token.to_string());
        url.set_path("/auth-chal-res");
        let res = self
            .client
            .put(url)
            .header("Authorization", token)
            .body(encode(&res)?)
            .send()
            .await
            .map_err(std::io::Error::other)?;
        if res.error_for_status_ref().is_err() {
            return Err(std::io::Error::other(
                res.text().await.map_err(std::io::Error::other)?,
            ));
        }

        self.set_api_token(req.token);

        Ok(())
    }

    async fn retry_auth<'a, 'b: 'a, 'c: 'a + 'b, R, F, C>(
        &'a self,
        url: reqwest::Url,
        c: C,
    ) -> Result<R>
    where
        F: std::future::Future<Output = Result<R>> + 'c + Send,
        C: Fn() -> F + 'b + Send,
    {
        if self.token.lock().unwrap().is_none() {
            self.get_auth_token(url.clone()).await?;
        }
        match c().await {
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                self.get_auth_token(url).await?;
                c().await
            }
            Err(e) => Err(e),
            Ok(r) => Ok(r),
        }
    }

    /// Execute a health check at the given url.
    pub async fn health(&self, url: &str) -> Result<()> {
        let mut url: reqwest::Url =
            url.parse().map_err(std::io::Error::other)?;
        url.set_path("health");
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

    /// Execute a context configuration command.
    pub async fn context(
        &self,
        url: &str,
        ctx: Hash,
        config: VmContextConfig,
    ) -> Result<()> {
        let data = encode(&config)?;
        let mut url: reqwest::Url =
            url.parse().map_err(std::io::Error::other)?;
        url.set_path(&format!("context/{ctx}"));
        self.retry_auth(url.clone(), move || {
            let url = url.clone();
            let data = data.clone();
            async move {
                let token = format!(
                    "Bearer {}",
                    &self.token.lock().unwrap().clone().unwrap().to_string()
                );
                let res = self
                    .client
                    .put(url)
                    .header("Authorization", token)
                    .body(data)
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
        })
        .await
    }

    /// Put data to a running VoidMerge server.
    pub async fn insert(
        &self,
        url: &str,
        ctx: Hash,
        data: Bytes,
    ) -> Result<()> {
        let mut url: reqwest::Url =
            url.parse().map_err(std::io::Error::other)?;
        url.set_path(&format!("insert/{ctx}"));
        self.retry_auth(url.clone(), move || {
            let url = url.clone();
            let data = data.clone();
            async move {
                let token = format!(
                    "Bearer {}",
                    &self.token.lock().unwrap().clone().unwrap().to_string()
                );
                let res = self
                    .client
                    .put(url)
                    .header("Authorization", token)
                    .body(data)
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
        })
        .await
    }

    /// Select data from a running VoidMerge server.
    pub async fn select(
        &self,
        url: &str,
        ctx: Hash,
        select: VmSelect,
    ) -> Result<VmSelectResponse> {
        let mut url: reqwest::Url =
            url.parse().map_err(std::io::Error::other)?;
        url.set_path(&format!("select/{ctx}"));
        self.retry_auth(url.clone(), move || {
            let url = url.clone();
            let select = select.clone();
            async move {
                let token = format!(
                    "Bearer {}",
                    &self.token.lock().unwrap().clone().unwrap().to_string()
                );
                let res = self
                    .client
                    .put(url)
                    .header("Authorization", token)
                    .body(encode(&select)?)
                    .send()
                    .await
                    .map_err(std::io::Error::other)?;
                if res.error_for_status_ref().is_err() {
                    return Err(std::io::Error::other(
                        res.text().await.map_err(std::io::Error::other)?,
                    ));
                }
                let res = res.bytes().await.map_err(std::io::Error::other)?;
                decode(&res)
            }
        })
        .await
    }
    */
}
