use std::sync::Arc;

use api_utils::ApiResult;
use axum::extract::State;

use crate::context::Context;

/// Get CID Lookup Table
#[utoipa::path(
    get,
    path = "/v1/db/tables/cid-lookup-table",
    tag = "/v1/db/tables/cid-lookup-table",
    responses(
        (status = 200, description = "Get CID Lookup Table", body = String)
    )
)]
pub async fn get_cid_lookup_table(State(ctx): State<Arc<Context>>) -> ApiResult<String> {
    let Context { db, .. } = &*ctx;

    let table = db.get_all_cid_lookups_ascii_table()?;

    Ok(table)
}
