use std::{path::Path, str::FromStr, sync::Arc};

use cid::Cid;
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::routes::{Route, RouteStub};

#[derive(Debug, Deserialize, Serialize)]
pub enum Direction {
    Asc,
    Desc,
}

impl FromStr for Direction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ASC" => Ok(Direction::Asc),
            "DESC" => Ok(Direction::Desc),
            _ => Err(format!("Invalid direction: {}", s)),
        }
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Asc => write!(f, "ASC"),
            Direction::Desc => write!(f, "DESC"),
        }
    }
}

pub enum OrderBy {
    CreatedAt(Direction),
    Size(Direction),
}

impl std::fmt::Display for OrderBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderBy::CreatedAt(d) => write!(f, "created_at {}", d),
            OrderBy::Size(d) => write!(f, "size {}", d),
        }
    }
}

#[derive(Debug)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub async fn open_or_create(db_path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Db {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.create_tables().await?;
        Ok(db)
    }

    pub async fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Db {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.create_tables().await?;
        Ok(db)
    }

    async fn create_tables(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        // Route table - you can add unique constraints as needed
        conn.execute(
            "CREATE TABLE IF NOT EXISTS routes (
                id TEXT PRIMARY KEY NOT NULL,
                created_at TEXT NOT NULL,
                verified_at TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                provider_type TEXT NOT NULL,
                url TEXT NOT NULL,
                cid BLOB,
                size INTEGER,
                creator BLOB,
                signature BLOB,
                multicodec TEXT,
                UNIQUE(provider_id, provider_type, cid),
                UNIQUE(provider_id, provider_type, url)
            )",
            [],
        )?;

        Ok(())
    }

    // Route operations
    pub async fn insert_route(&self, route: &Route) -> Result<()> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "INSERT INTO routes (id, created_at, verified_at, provider_id, provider_type, url, cid, size, multicodec, creator, signature)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
        )?;

        // TODO(b5) - remove unwraps!
        let created = route.created_at.format(&Rfc3339).unwrap();
        let verified_at = route.verified_at.format(&Rfc3339).unwrap();

        stmt.execute(params![
            route.id.to_string(),
            created,
            verified_at,
            route.provider_type.to_string(),
            route.provider_type.to_string(),
            route.url,
            route.cid.to_bytes(),
            route.size as i64,
            route.multicodec.to_string(),
            route.creator.as_bytes(),
            route.signature,
        ])?;

        Ok(())
    }

    pub async fn list_routes(
        &self,
        order_by: OrderBy,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Route>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, created_at, verified_at, provider_id, provider_type, url, cid, size, multicodec, creator, signature
             FROM routes
             WHERE cid is not null
             ORDER BY ?1 DESC
             LIMIT ?2 OFFSET ?3
             ",
        )?;

        let route_iter = stmt.query_map(
            params![order_by.to_string(), limit, offset],
            Route::from_sql_row,
        )?;

        let routes = route_iter.collect::<Result<Vec<Route>>>()?;

        Ok(routes)
    }

    pub async fn list_provider_routes(
        &self,
        provider_id: &str,
        order_by: OrderBy,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Route>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, created_at, verified_at, provider_id, provider_type, url, cid, size, multicodec, creator, signature
             FROM routes
             WHERE cid is not null
             AND provider_id = ?1
             ORDER BY ?2
             LIMIT ?3 OFFSET ?4
             ",
        )?;

        let route_iter = stmt.query_map(
            params![provider_id, order_by.to_string(), limit, offset],
            Route::from_sql_row,
        )?;

        let routes = route_iter.collect::<Result<Vec<Route>>>()?;

        Ok(routes)
    }

    pub async fn get_route(&self, id: Uuid) -> Result<Option<Route>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, created_at, verified_at, provider_id, provider_type, url, cid, size, multicodec, creator, signature
             FROM routes WHERE id = ?1 AND cid is not null",
        )?;

        let result = stmt.query_row(params![id.to_string()], Route::from_sql_row);

        match result {
            Ok(route) => Ok(Some(route)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn routes_for_cid(&self, cid: Cid) -> Result<Vec<Route>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, created_at, verified_at, provider_id, provider_type, url, cid, size, multicodec, creator, signature
             FROM routes
             WHERE cid = ?1
             AND cid IS NOT NULL
             LIMIT 1",
        )?;

        let route_iter = stmt.query_map(params![cid.to_bytes()], Route::from_sql_row)?;

        let mut routes = Vec::new();
        for route in route_iter {
            routes.push(route?);
        }
        Ok(routes)
    }

    pub async fn routes_for_url(&self, url: &str) -> Result<Vec<Route>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, created_at, verified_at, provider_id, provider_type, url, cid, size, multicodec, creator, signature
             FROM routes
             WHERE url = ?1
             AND cid IS NOT NULL
             LIMIT 1",
        )?;

        let route_iter = stmt.query_map(params![url], Route::from_sql_row)?;

        let mut routes = Vec::new();
        for route in route_iter {
            routes.push(route?);
        }
        Ok(routes)
    }

    pub async fn all_routes(
        &self,
        order_by: OrderBy,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Route>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, created_at, verified_at, provider_id, provider_type, url, cid, size, multicodec, creator, signature
             FROM routes
             WHERE cid IS NOT NULL
             ORDER BY ?1
             LIMIT ?2 OFFSET ?3",
        )?;

        let route_iter = stmt.query_map(
            params![order_by.to_string(), limit, offset],
            Route::from_sql_row,
        )?;

        let mut routes = Vec::new();
        for route in route_iter {
            routes.push(route?);
        }
        Ok(routes)
    }

    /// list stubs for a given provider
    pub async fn list_provider_stubs(
        &self,
        provider_id: &str,
        order_by: OrderBy,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<RouteStub>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, created_at, verified_at, provider_id, provider_type, url, cid, size, multicodec, creator, signature
             FROM routes
             WHERE provider_id = ?1
             ORDER BY ?2
             LIMIT ?3 OFFSET ?4",
        )?;

        let route_iter = stmt.query_map(
            params![provider_id, order_by.to_string(), limit, offset],
            RouteStub::from_sql_row,
        )?;

        let mut stubs = Vec::new();
        for stub in route_iter {
            stubs.push(stub?);
        }
        Ok(stubs)
    }

    pub async fn insert_stub(&self, stub: &RouteStub) -> Result<()> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "INSERT INTO routes (id, created_at, verified_at, provider_id, provider_type, url, cid, size, multicodec, creator, signature)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?;

        // TODO(b5) - remove unwraps!
        let created_at = stub.created_at.format(&Rfc3339).unwrap();
        let verified_at = stub.verified_at.format(&Rfc3339).unwrap();

        stmt.execute(params![
            stub.id.to_string(),
            created_at,
            verified_at,
            stub.provider_id,
            stub.provider_type.to_string(),
            stub.url,
            None::<Vec<u8>>,                                  // cid
            stub.size,                                        // size
            stub.multicodec.map(|format| format.to_string()), // multicodec
            None::<Vec<u8>>,                                  // creator
            None::<Vec<u8>>,                                  // signature
        ])?;

        Ok(())
    }

    pub async fn complete_stub(&self, route: &Route) -> Result<()> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "UPDATE routes
                SET verified_at = ?2, provider_id = ?3, provider_type = ?4, url = ?5,
                cid = ?6, size = ?7, multicodec = ?8, creator = ?9, signature = ?10
                WHERE id = ?1",
        )?;

        let verified_at = route.verified_at.format(&Rfc3339).unwrap();

        stmt.execute(params![
            route.id.to_string(),
            verified_at,
            route.provider_id,
            route.provider_type.to_string(),
            route.url,
            route.cid.to_bytes(),
            route.size as i64,
            route.multicodec.to_string(),
            route.creator.as_bytes(),
            route.signature,
        ])?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use cid::Cid;

    use super::*;
    use crate::{
        auth::Auth,
        cid::Codec,
        cid_filter::CidFilter,
        crp::{Crp, CrpCapabilities, ProviderType},
        Context,
    };

    struct StubAzureProvider {}

    #[async_trait]
    impl Crp for StubAzureProvider {
        fn provider_id(&self) -> String {
            "azure".to_string()
        }

        fn provider_type(&self) -> ProviderType {
            ProviderType::Azure
        }

        async fn reindex(&self, _cx: &Context) -> anyhow::Result<()> {
            todo!();
        }

        fn capabilities(&self) -> CrpCapabilities<'_> {
            CrpCapabilities {
                route_resolver: None,
                size_resolver: None,
            }
        }

        fn cid_filter(&self) -> crate::cid_filter::CidFilter {
            CidFilter::None
        }
    }

    #[tokio::test]
    async fn test_route_persistence() {
        let ctx = Context::mem(Auth::None).await.unwrap();
        let db = Db::new_in_memory().await.unwrap();
        let provider = StubAzureProvider {};

        // Test Route
        let cid =
            Cid::try_from("bafkreibme22gw2h7y2h7tg2fhqotaqjucnbc24deqo72b6mkl2egezxhvy").unwrap();

        let route = Route::builder(&provider)
            .cid(cid)
            .size(1024)
            .url("/test/route".to_string())
            .multicodec(Codec::Raw)
            .build(&ctx)
            .unwrap();

        db.insert_route(&route).await.unwrap();

        let routes = db
            .list_routes(OrderBy::CreatedAt(Direction::Desc), 0, 10000)
            .await
            .unwrap();
        assert_eq!(routes.len(), 1);

        let retrieved_route = db.get_route(route.id).await.unwrap().unwrap();
        assert_eq!(retrieved_route.cid, route.cid);

        let retrieved_routes = db.routes_for_url(&route.url).await.unwrap();
        assert_eq!(retrieved_routes.len(), 1);
        assert_eq!(retrieved_routes[0].cid, route.cid);
    }

    #[tokio::test]
    async fn test_stubs() {
        let ctx = Context::mem(Auth::None).await.unwrap();
        let db = Db::new_in_memory().await.unwrap();
        let provider = StubAzureProvider {};

        let stub = Route::builder(&provider)
            .url("/test/route".to_string())
            .build_stub()
            .unwrap();

        // sanity check
        let routes = db
            .list_provider_routes(
                &provider.provider_id(),
                OrderBy::CreatedAt(Direction::Desc),
                0,
                -1,
            )
            .await
            .unwrap();
        assert_eq!(routes.len(), 0);

        db.insert_stub(&stub).await.unwrap();

        let retrieved_routes = db.routes_for_url(&stub.url).await.unwrap();
        assert_eq!(retrieved_routes.len(), 0);

        let stubs = db
            .list_provider_stubs(
                &provider.provider_id(),
                OrderBy::CreatedAt(Direction::Desc),
                0,
                10000,
            )
            .await
            .unwrap();
        assert_eq!(stubs.len(), 1);

        assert_eq!(stubs[0].id, stub.id);

        let cid =
            Cid::try_from("bafkreibme22gw2h7y2h7tg2fhqotaqjucnbc24deqo72b6mkl2egezxhvy").unwrap();

        let route = stubs[0]
            .builder()
            .cid(cid)
            .size(1024)
            .multicodec(Codec::Raw)
            .build(&ctx)
            .unwrap();

        db.complete_stub(&route).await.unwrap();

        let retrieved_routes = db
            .all_routes(OrderBy::CreatedAt(Direction::Asc), 0, 10000)
            .await
            .unwrap();
        assert_eq!(retrieved_routes.len(), 1);

        let retrieved_routes = db.routes_for_url(&route.url).await.unwrap();
        assert_eq!(retrieved_routes.len(), 1);
        assert_eq!(retrieved_routes[0].cid, route.cid);
    }
}
