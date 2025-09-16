use uuid::Uuid;

use crate::crp::ProviderType;

/// An authentication token for a specific provider.
pub struct AuthToken {
    pub id: Uuid,
    pub provider_type: ProviderType,
    pub token: Vec<u8>,
}

impl AuthToken {
    pub fn new(provider_type: ProviderType, token: Vec<u8>) -> Self {
        AuthToken {
            id: Uuid::new_v4(),
            provider_type,
            token,
        }
    }

    pub(crate) fn from_sql_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        // TODO(b5) - remove unwraps
        let id = row.get::<_, String>(0)?;
        let id = Uuid::parse_str(&id).unwrap();
        let provider_type = row.get::<_, String>(1)?;
        let provider_type = ProviderType::from_str(&provider_type).unwrap();
        let token = row.get::<_, Vec<u8>>(2)?;
        Ok(AuthToken {
            id,
            provider_type,
            token,
        })
    }
}

// extracts token bytes from an authentication token, replacing None-type responses with an empty vector
pub fn token_bytes(auth_token: Option<AuthToken>) -> Vec<u8> {
    match auth_token {
        Some(token) => token.token,
        None => Vec::new(),
    }
}
