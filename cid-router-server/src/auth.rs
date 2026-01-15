use std::{
    fmt::Debug,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[async_trait]
pub trait AuthService: Send + Sync + Debug {
    async fn authenticate(&self, token: Option<String>) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EqtyJwt {
    pub jwks_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Auth {
    // No authentication: this will allow any user to access the API
    #[default]
    None,
    // EQTYLab variation of JWT authentication
    EqtyJwt(EqtyJwt),
}

impl Auth {
    pub async fn service(&self) -> Box<dyn AuthService> {
        match self {
            Auth::None => Box::new(NoneAuth),
            Auth::EqtyJwt(EqtyJwt { jwks_url }) => {
                // Implement JWT authentication logic here
                Box::new(EqtyAuthClient::new(jwks_url.clone()))
            }
        }
    }
}

#[derive(Debug)]
struct NoneAuth;

#[async_trait]
impl AuthService for NoneAuth {
    async fn authenticate(&self, _token: Option<String>) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Jwk {
    kid: String,
    // kty: String,
    // r#use: Option<String>,
    n: String, // RSA modulus
    e: String, // RSA exponent
}

#[derive(Debug, Clone, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug)]
struct JwksCache {
    jwks: Jwks,
    fetched_at: Instant,
    ttl: Duration,
}

// TODO(b5) - match these to EQTYLab RBAC spec
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    iat: usize,
    email: Option<String>,
}

#[derive(Debug)]
struct EqtyAuthClient {
    url: String,
    cache: Arc<RwLock<Option<JwksCache>>>,
}

#[async_trait]
impl AuthService for EqtyAuthClient {
    async fn authenticate(&self, token: Option<String>) -> Result<()> {
        let token = token.ok_or(anyhow!("Token is missing"))?;

        let header = decode_header(&token)?;
        let kid = header.kid.ok_or(anyhow!("Token doesn't have a kid"))?;

        let jwks = self.get_jwks().await?;
        let jwk = Self::find_jwk(&jwks, &kid).ok_or(anyhow!("No matching key found in JWKS"))?;

        let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)?;
        let validation = Validation::new(Algorithm::RS256);

        let _token_data = decode::<Claims>(&token, &decoding_key, &validation)?;
        // TODO: validate claims
        Ok(())
    }
}

impl EqtyAuthClient {
    fn new(url: String) -> Self {
        Self {
            url,
            cache: Arc::new(RwLock::new(None)),
        }
    }

    fn find_jwk<'a>(jwks: &'a Jwks, kid: &str) -> Option<&'a Jwk> {
        jwks.keys.iter().find(|k| k.kid == kid)
    }

    async fn fetch_jwks(&self) -> Result<Jwks> {
        let response = reqwest::get(&self.url).await?;
        let jwks: Jwks = response.json().await?;
        Ok(jwks)
    }

    async fn get_jwks(&self) -> Result<Jwks> {
        let cache = self.cache.read().await;

        // Check if cache is valid
        if let Some(cached) = cache.as_ref() {
            if cached.fetched_at.elapsed() < cached.ttl {
                return Ok(cached.jwks.clone());
            }
        }
        drop(cache);

        // Fetch new JWKS
        let jwks = self.fetch_jwks().await?;

        // Update cache
        let mut cache = self.cache.write().await;
        *cache = Some(JwksCache {
            jwks: jwks.clone(),
            fetched_at: Instant::now(),
            ttl: Duration::from_secs(3600), // 1 hour
        });

        Ok(jwks)
    }
}

// mod tests {
//     use super::*;

//     #[test]
//     fn test_verify_jwt() {
//         let token_str = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJzb21lb25lIn0.5wwE1sBrs-vftww_BGIuTVDeHtc1Jsjo-fiHhDwR8m0";
//         Auth::EqtyJwt.authenticate(token);
//         let claims = verify_jwt(token_str, &Hmac::new_from_slice(b"some-secret").unwrap()).unwrap();
//         assert_eq!(claims["sub"], "someone");
//     }
// }
