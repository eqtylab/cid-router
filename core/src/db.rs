use rusqlite::{params, Connection, Result};
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::routes::{Route, RouteStub};

pub enum Direction {
    Asc,
    Desc,
}

impl ToString for Direction {
    fn to_string(&self) -> String {
        match self {
            Direction::Asc => "ASC".to_string(),
            Direction::Desc => "DESC".to_string(),
        }
    }
}

pub enum OrderBy {
    CreatedAt(Direction),
    Size(Direction),
}

impl ToString for OrderBy {
    fn to_string(&self) -> String {
        match self {
            OrderBy::CreatedAt(_) => "created_at".to_string(),
            OrderBy::Size(_) => "size".to_string(),
        }
    }
}

impl OrderBy {
    fn direction_string(&self) -> String {
        match self {
            OrderBy::CreatedAt(dir) => dir.to_string(),
            OrderBy::Size(dir) => dir.to_string(),
        }
    }
}

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
        // Route table - you can add unique constraints as needed
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS routes (
                id TEXT PRIMARY KEY NOT NULL,
                created_at TEXT NOT NULL,
                verified_at TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                provider_type TEXT NOT NULL,
                route TEXT NOT NULL,
                cid BLOB,
                size INTEGER,
                creator BLOB,
                signature BLOB,
                blob_format TEXT,
                UNIQUE(provider_id, provider_type, cid)
            )",
            [],
        )?;

        Ok(())
    }

    // Route operations
    pub fn insert_route(&self, route: &Route) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO routes (id, created_at, verified_at, provider_id, provider_type, route, cid, size, blob_format, creator, signature)
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
            route.route,
            route.cid.to_bytes(),
            route.size as i64,
            route.blob_format.to_string(),
            route.creator.as_bytes(),
            route.signature,
        ])?;

        Ok(())
    }

    pub fn list_routes(&self, offset: i64, limit: i64) -> Result<Vec<Route>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, verified_at, provider_id, provider_type, route, cid, size, blob_format, creator, signature
             FROM routes
             WHERE cid is not null
             ORDER BY created_at DESC
             LIMIT ?1 OFFSET ?2
             ",
        )?;

        let route_iter = stmt.query_map(params![limit, offset], Route::from_sql_row)?;

        let routes = route_iter.collect::<Result<Vec<Route>>>()?;

        Ok(routes)
    }

    pub fn list_provider_routes(
        &self,
        provider_id: &str,
        order_by: OrderBy,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Route>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, verified_at, provider_id, provider_type, route, cid, size, blob_format, creator, signature
             FROM routes
             WHERE cid is not null
             AND provider_id = ?1
             ORDER BY ?2 ?3
             LIMIT ?4 OFFSET ?5
             ",
        )?;

        let route_iter = stmt.query_map(
            params![
                provider_id,
                order_by.column_name(),
                order_by.direction_string(),
                limit,
                offset
            ],
            Route::from_sql_row,
        )?;

        let routes = route_iter.collect::<Result<Vec<Route>>>()?;

        Ok(routes)
    }

    pub fn get_route(&self, id: Uuid) -> Result<Option<Route>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, verified_at, provider_id, provider_type, route, cid, size, blob_format, creator, signature
             FROM routes WHERE id = ?1 AND cid is not null",
        )?;

        let result = stmt.query_row(params![id.to_string()], Route::from_sql_row);

        match result {
            Ok(route) => Ok(Some(route)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn routes_for_url(&self, url: &str) -> Result<Vec<Route>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, verified_at, provider_id, provider_type, route, cid, size, blob_format, creator, signature
             FROM routes
             WHERE route = ?1
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

    pub fn all_routes(&self, order_by: OrderBy, offset: i64, limit: i64) -> Result<Vec<Route>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, verified_at, provider_id, provider_type, route, cid, size, blob_format, creator, signature
             FROM routes
             WHERE cid IS NOT NULL
             ORDER BY ?1 ?2
             LIMIT ?3 OFFSET ?4",
        )?;

        let route_iter = stmt.query_map(
            params![
                order_by.to_string(),
                order_by.direction_string(),
                limit,
                offset
            ],
            Route::from_sql_row,
        )?;

        let mut routes = Vec::new();
        for route in route_iter {
            routes.push(route?);
        }
        Ok(routes)
    }

    /// list stubs for a given provider
    pub fn list_provider_stubs(
        &self,
        provider_id: &str,
        order_by: OrderBy,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<RouteStub>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, verified_at, provider_id, provider_type, route, cid, size, blob_format, creator, signature
             FROM routes
             WHERE provider_id = ?1
             ORDER BY ?2 ?3
             LIMIT ?4 OFFSET ?5",
        )?;

        let route_iter = stmt.query_map(
            params![
                provider_id,
                order_by.to_string(),
                order_by.direction_string(),
                limit,
                offset
            ],
            RouteStub::from_sql_row,
        )?;

        let mut stubs = Vec::new();
        for stub in route_iter {
            stubs.push(stub?);
        }
        Ok(stubs)
    }

    pub fn insert_stub(&self, stub: &RouteStub) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO routes (id, created_at, verified_at, provider_id, provider_type, route, cid, size, blob_format, creator, signature)
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
            stub.route,
            None::<Vec<u8>>, // cid
            None::<i64>,     // size
            None::<String>,  // blob_format
            None::<Vec<u8>>, // creator
            None::<Vec<u8>>, // signature
        ])?;

        Ok(())
    }

    pub fn complete_stub(&self, route: &Route) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "UPDATE routes
                SET verified_at = ?2, provider_id = ?3, provider_type = ?4, route = ?5,
                cid = ?6, size = ?7, blob_format = ?8, creator = ?9, signature = ?10
                WHERE id = ?1",
        )?;

        let verified_at = route.verified_at.format(&Rfc3339).unwrap();

        stmt.execute(params![
            route.id.to_string(),
            verified_at,
            route.provider_id,
            route.provider_type.to_string(),
            route.route,
            route.cid.to_bytes(),
            route.size as i64,
            route.blob_format.to_string(),
            route.creator.as_bytes(),
            route.signature,
        ])?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::crp::ProviderType;
    use crate::{crp::Provider, Context};
    use cid::Cid;
    use iroh_blobs::BlobFormat;

    use super::*;

    struct StubAzureProvider {}

    impl Provider for StubAzureProvider {
        fn provider_id(&self) -> String {
            "azure".to_string()
        }

        fn provider_type(&self) -> ProviderType {
            ProviderType::Azure
        }
    }

    #[test]
    fn test_route_persistence() {
        let ctx = Context::new().unwrap();
        let db = Db::new_in_memory().unwrap();
        let provider = StubAzureProvider {};

        // Test Route
        let cid =
            Cid::try_from("bafkreibme22gw2h7y2h7tg2fhqotaqjucnbc24deqo72b6mkl2egezxhvy").unwrap();

        let route = Route::builder(&provider)
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

        let retrieved_routes = db.routes_for_url(&route.route).unwrap();
        assert_eq!(retrieved_routes.len(), 1);
        assert_eq!(retrieved_routes[0].cid, route.cid);
    }

    #[test]
    fn test_stubs() {
        let ctx = Context::new().unwrap();
        let db = Db::new_in_memory().unwrap();
        let provider = StubAzureProvider {};

        let stub = Route::builder(&provider)
            .route("/test/route".to_string())
            .build_stub()
            .unwrap();

        db.insert_stub(&stub).unwrap();

        let retrieved_routes = db.routes_for_url(&stub.route).unwrap();
        assert_eq!(retrieved_routes.len(), 0);

        let stubs = db
            .list_provider_stubs(
                &provider.provider_id(),
                OrderBy::CreatedAt(Direction::Desc),
                0,
                10000,
            )
            .unwrap();
        assert_eq!(stubs.len(), 1);

        assert_eq!(stubs[0].id, stub.id);

        let cid =
            Cid::try_from("bafkreibme22gw2h7y2h7tg2fhqotaqjucnbc24deqo72b6mkl2egezxhvy").unwrap();

        let route = stubs[0]
            .builder()
            .cid(cid)
            .size(1024)
            .format(BlobFormat::Raw)
            .build(&ctx)
            .unwrap();

        db.complete_stub(&route).unwrap();

        let retrieved_routes = db
            .all_routes(OrderBy::CreatedAt(Direction::Asc), 0, 10000)
            .unwrap();
        assert_eq!(retrieved_routes.len(), 1);

        let retrieved_routes = db.routes_for_url(&route.route).unwrap();
        assert_eq!(retrieved_routes.len(), 1);
        assert_eq!(retrieved_routes[0].cid, route.cid);
    }
}
