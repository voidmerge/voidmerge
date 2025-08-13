use super::*;

/// Closure for making an atomic edit of the runtime store.
pub type RuntimeStoreAtomicEdit<'lt> =
    Box<dyn FnOnce(Option<Arc<str>>) -> Result<Option<Arc<str>>> + 'lt + Send>;

/// Factory for a [ModuleRuntimeStore] module.
pub trait ModuleRuntimeStoreFactory:
    std::fmt::Debug + 'static + Send + Sync
{
    /// Create the [ModuleRuntimeStore] module.
    fn factory(
        &self,
        config: Arc<config::Config>,
    ) -> BoxFut<'static, Result<DynModuleRuntimeStore>>;
}

/// Trait object [ModuleRuntimeStoreFactory].
pub type DynModuleRuntimeStoreFactory =
    Arc<dyn ModuleRuntimeStoreFactory + 'static + Send + Sync>;

/// Defines a module that is a runtime KV store.
pub trait ModuleRuntimeStore: std::fmt::Debug + 'static + Send + Sync {
    /// Set a value in the runtime store.
    /// Returns 'true' if the value was set, and the current value.
    fn set<'a, 'b: 'a>(
        &'a self,
        k: Arc<str>,
        edit: RuntimeStoreAtomicEdit<'b>,
    ) -> BoxFut<'a, Result<(bool, Option<Arc<str>>)>>;

    /// List the keys from the runtime store.
    fn list(&self) -> Vec<Arc<str>>;

    /// Get a value from the runtime store.
    fn get(&self, k: &str) -> Option<Arc<str>>;

    // -- provided -- //

    /// Register a context and related metadata.
    fn context_register<'a, 'b: 'a>(
        &'a self,
        ctx: Hash,
        meta: Value,
    ) -> BoxFut<'a, Result<()>> {
        Box::pin(async move {
            let val = serde_json::to_string(&meta)?;
            self.set(
                format!("ctx-{ctx}").into(),
                Box::new(move |_| Ok(Some(val.into()))),
            )
            .await?;
            Ok(())
        })
    }

    /// List the registered contexts.
    fn context_list(&self) -> Result<Vec<(Hash, Value)>> {
        let mut out = Vec::new();
        for key in self.list() {
            if key.starts_with("ctx-") {
                let val = match self.get(&key) {
                    None => continue,
                    Some(v) => v,
                };
                let val = serde_json::from_str(&val)?;
                let key = key.trim_start_matches("ctx-");
                let key: Hash = key.parse()?;
                out.push((key, val));
            }
        }
        Ok(out)
    }

    /// Ensure a keypair exists for a given signing algorithm module.
    fn assert_sign_keypair<'a, 'b: 'a>(
        &'a self,
        sign: &'b DynModuleSign,
    ) -> BoxFut<'a, Result<(SignSecretKey, SignPublicKey)>> {
        Box::pin(async move {
            let alg = sign.alg();
            let (_did_set, sk) = self
                .set(
                    format!("sign-{alg}-secret").into(),
                    Box::new(|cur| {
                        if cur.is_some() {
                            return Ok(None);
                        }
                        Ok(Some(sign.gen_secret().to_string().into()))
                    }),
                )
                .await?;
            let sk = sk.expect("failed to generate secret key");
            let sk: SignSecretKey = sk.parse()?;
            let (_did_set, pk) = self
                .set(
                    format!("sign-{alg}-public").into(),
                    Box::new(|_| {
                        Ok(Some(sign.gen_public(&sk).to_string().into()))
                    }),
                )
                .await?;
            let pk = pk.expect("failed to generate public key");
            let pk: SignPublicKey = pk.parse()?;
            Ok((sk, pk))
        })
    }
}

/// Trait object [ModuleRuntimeStore].
pub type DynModuleRuntimeStore =
    Arc<dyn ModuleRuntimeStore + 'static + Send + Sync>;
