use cid::Cid;
use iroh::PublicKey;
use iroh_blobs::BlobFormat;
use rusqlite::{params, Connection, Result};
use time::{format_description::well_known::Rfc3339, OffsetDateTime as DateTime};
use uuid::Uuid;

use crate::{auth::AuthToken, crp::ProviderType, routes::Route};

#[derive(Debug)]
pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Db { conn };
        db.create_tables()?;
        Ok(db)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Db { conn };
        db.create_tables()?;
        Ok(db)
    }

    fn create_tables(&self) -> Result<()> {
        // AuthToken table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS auth_tokens (
                id TEXT PRIMARY KEY NOT NULL,
                provider TEXT NOT NULL,
                token TEXT NOT NULL,
                UNIQUE(provider, token)
            )",
            [],
        )?;

        // Route table - you can add unique constraints as needed
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS routes (
                id TEXT PRIMARY KEY NOT NULL,
                provider TEXT NOT NULL,
                cid BLOB NOT NULL,
                size INTEGER NOT NULL,
                route TEXT NOT NULL,
                creator BLOB NOT NULL,
                signature BLOB NOT NULL,
                created_at TEXT NOT NULL,
                verified_at TEXT NOT NULL,
                blob_format TEXT NOT NULL,
                UNIQUE(provider, cid)
            )",
            [],
        )?;

        Ok(())
    }

    // Did operations
    pub fn insert_auth_token(&self, token: &AuthToken) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("INSERT INTO auth_tokens (id, provider, token) VALUES (?1, ?2, ?3)")?;

        stmt.execute(params![
            token.id.to_string(),
            token.provider_type.to_string(),
            token.token.clone(),
        ])?;

        Ok(())
    }

    pub fn auth_token_for_provider(&self, provider: ProviderType) -> Result<Option<AuthToken>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, provider, token FROM auth_tokens WHERE provider = ?1")?;

        let result = stmt.query_row(params![provider.to_string()], AuthToken::from_sql_row);

        match result {
            Ok(did) => Ok(Some(did)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // Route operations
    pub fn insert_route(&self, route: &Route) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO routes (id, provider, cid, size, route, creator, signature, created_at, verified_at, blob_format)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"
        )?;

        // TODO(b5) - remove unwraps!
        let created = route.created_at.format(&Rfc3339).unwrap();
        let verified_at = route.verified_at.format(&Rfc3339).unwrap();

        stmt.execute(params![
            route.id.to_string(),
            route.provider.to_string(),
            route.cid.to_bytes(),
            route.size as i64,
            route.route,
            route.creator.as_bytes(),
            route.signature,
            created,
            verified_at,
            route.blob_format.to_string(),
        ])?;

        Ok(())
    }

    pub fn list_routes(&self, offset: u64, limit: u64) -> Result<Vec<Route>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, provider, cid, size, route, creator, signature, created_at, verified_at, blob_format
             FROM routes ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
        )?;

        let route_iter = stmt.query_map(params![limit, offset], Self::route_from_sql_row)?;

        let routes = route_iter.collect::<Result<Vec<Route>>>()?;

        Ok(routes)
    }

    pub fn get_route(&self, id: Uuid) -> Result<Option<Route>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, provider, cid, size, route, creator, signature, created_at, verified_at, blob_format
             FROM routes WHERE id = ?1",
        )?;

        let result = stmt.query_row(params![id.to_string()], Self::route_from_sql_row);

        match result {
            Ok(route) => Ok(Some(route)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn routes_for_url(&self, store_type: ProviderType, url: String) -> Result<Vec<Route>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, provider, cid, size, route, creator, signature, created_at, verified_at, blob_format
             FROM routes WHERE provider = ?1 AND route = ?2 LIMIT 1",
        )?;

        let route_iter = stmt.query_map(
            params![store_type.to_string(), url],
            Self::route_from_sql_row,
        )?;

        let mut routes = Vec::new();
        for route in route_iter {
            routes.push(route?);
        }
        Ok(routes)
    }

    fn route_from_sql_row(row: &rusqlite::Row<'_>) -> Result<Route> {
        // TODO(b5) - remove unwraps!
        let data = row.get::<_, Vec<u8>>(2)?;
        let cid = Cid::try_from(data).unwrap();

        let blob_format_str: String = row.get(9)?;
        let blob_format = match blob_format_str.as_str() {
            "Raw" => BlobFormat::Raw,
            "HashSeq" => BlobFormat::HashSeq,
            _ => BlobFormat::Raw, // default fallback
        };

        let id = row.get::<_, String>(0)?;
        // TODO(b5) - remove unwarp
        let id = Uuid::parse_str(&id).unwrap();

        let pub_key = row.get::<_, [u8; 32]>(5)?;
        // TODO(b5) - remove unwarp
        let creator = PublicKey::from_bytes(&pub_key).unwrap();

        Ok(Route {
            id,
            provider: ProviderType::from_str(&row.get::<_, String>(1)?).unwrap(),
            cid,
            size: row.get::<_, i64>(3)? as u64,
            route: row.get(4)?,
            creator,
            signature: row.get(6)?,
            created_at: DateTime::parse(&row.get::<_, String>(7)?, &Rfc3339).unwrap(),
            verified_at: DateTime::parse(&row.get::<_, String>(8)?, &Rfc3339).unwrap(),
            blob_format,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Context;

    use super::*;

    #[test]
    fn test_basic_operations() {
        let ctx = Context::new().unwrap();
        let db = Db::new_in_memory().unwrap();

        // Test AuthToken
        let token = AuthToken::new(
            ProviderType::Azure,
            b"such_secret_many_token_much_wow".to_vec(),
        );

        db.insert_auth_token(&token).unwrap();
        let retrieved_token = db
            .auth_token_for_provider(ProviderType::Azure)
            .unwrap()
            .unwrap();
        assert_eq!(retrieved_token.token, token.token);

        // Test Route
        let cid =
            Cid::try_from("bafkreibme22gw2h7y2h7tg2fhqotaqjucnbc24deqo72b6mkl2egezxhvy").unwrap();

        let route = Route::builder(ProviderType::Iroh)
            .cid(cid)
            .size(1024)
            .route("/test/route".to_string())
            .format(BlobFormat::Raw)
            .build(&ctx)
            .unwrap();

        db.insert_route(&route).unwrap();

        let routes = db.list_routes(0, 10000).unwrap();
        assert_eq!(routes.len(), 1);

        let retrieved_route = db.get_route(route.id).unwrap().unwrap();
        assert_eq!(retrieved_route.cid, route.cid);
    }
}
