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

    /// Call the admin obj-list api on a VoidMerge server.
    pub async fn obj_list(
        &self,
        url: &str,
        ctx: &str,
        token: &str,
        app_path_prefix: &str,
        created_gt: f64,
        limit: u32,
    ) -> Result<Vec<crate::obj::ObjMeta>> {
        safe_str(ctx)?;
        safe_str(app_path_prefix)?;
        let mut url: reqwest::Url =
            url.parse().map_err(std::io::Error::other)?;
        url.set_path(&format!("{ctx}/_vm_/obj-list/{app_path_prefix}"));
        url.query_pairs_mut()
            .clear()
            .append_pair("created-gt", &created_gt.to_string())
            .append_pair("limit", &limit.to_string());
        let token = format!("Bearer {}", &token);
        let res = self
            .client
            .get(url)
            .header("Authorization", token)
            .send()
            .await
            .map_err(std::io::Error::other)?;
        if res.error_for_status_ref().is_err() {
            return Err(std::io::Error::other(
                res.text().await.map_err(std::io::Error::other)?,
            ));
        }
        let res = res.bytes().await.map_err(std::io::Error::other)?;
        #[derive(serde::Deserialize)]
        struct R {
            #[serde(rename = "metaList")]
            meta_list: Vec<crate::obj::ObjMeta>,
        }
        let res: R = res.to_decode()?;
        Ok(res.meta_list)
    }

    /// Call the admin obj-get api on a VoidMerge server.
    pub async fn obj_get(
        &self,
        url: &str,
        ctx: &str,
        token: &str,
        app_path: &str,
    ) -> Result<(crate::obj::ObjMeta, bytes::Bytes)> {
        safe_str(ctx)?;
        safe_str(app_path)?;
        let mut url: reqwest::Url =
            url.parse().map_err(std::io::Error::other)?;
        url.set_path(&format!("{ctx}/_vm_/obj-get/{app_path}"));
        let token = format!("Bearer {}", &token);
        let res = self
            .client
            .get(url)
            .header("Authorization", token)
            .send()
            .await
            .map_err(std::io::Error::other)?;
        if res.error_for_status_ref().is_err() {
            return Err(std::io::Error::other(
                res.text().await.map_err(std::io::Error::other)?,
            ));
        }
        let res = res.bytes().await.map_err(std::io::Error::other)?;
        #[derive(serde::Deserialize)]
        struct R {
            meta: crate::obj::ObjMeta,
            data: bytes::Bytes,
        }
        let res: R = res.to_decode()?;
        Ok((res.meta, res.data))
    }

    /// Call the admin obj-put api on a VoidMerge server.
    #[allow(clippy::too_many_arguments)]
    pub async fn obj_put(
        &self,
        url: &str,
        ctx: &str,
        token: &str,
        app_path: &str,
        created_secs: f64,
        expires_secs: f64,
        data: bytes::Bytes,
    ) -> Result<crate::obj::ObjMeta> {
        safe_str(ctx)?;
        safe_str(app_path)?;
        let mut url: reqwest::Url =
            url.parse().map_err(std::io::Error::other)?;
        url.set_path(&format!(
            "{ctx}/_vm_/obj-put/{app_path}/{created_secs}/{expires_secs}"
        ));
        let token = format!("Bearer {}", &token);
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
        let res = res.text().await.map_err(std::io::Error::other)?;
        Ok(crate::obj::ObjMeta(res.into()))
    }
}
