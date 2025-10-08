use std::sync::Arc;

use anyhow::Result;
use iroh::{PublicKey, SecretKey};
use iroh_base::Signature;

use crate::{
    auth::{Auth, AuthService},
    db::Db,
    repo::Repo,
};

// Context bundles shared state to pass around different parts of a program,
// like CID Route Providers (CRPs) and API wrappers. It bundles identity
// database access, and shared configuration
#[derive(Debug, Clone)]
pub struct Context {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    key: SecretKey,
    db: Db,
    auth: Box<dyn AuthService>,
}

impl Context {
    pub async fn from_repo(repo: Repo, auth: Auth) -> Result<Self> {
        let db = repo.db().await?;
        let key = repo.secret_key().await?;
        let auth = auth.service().await;

        let inner = Inner { db, key, auth };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub async fn mem(auth: Auth) -> Result<Self> {
        let db = Db::new_in_memory().await?;
        let key = SecretKey::generate(rand::rngs::OsRng);
        let auth = auth.service().await;
        let inner = Inner { db, key, auth };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub fn db(&self) -> &Db {
        &self.inner.db
    }

    pub async fn authenticate(&self, token: Option<String>) -> Result<()> {
        self.inner.auth.authenticate(token).await
    }
}

impl Signer for Context {
    fn public_key(&self) -> PublicKey {
        self.inner.key.public()
    }

    fn sign(&self, data: &[u8]) -> Signature {
        self.inner.key.sign(data)
    }
}

pub trait Signer {
    fn public_key(&self) -> PublicKey;
    fn sign(&self, data: &[u8]) -> Signature;
}
