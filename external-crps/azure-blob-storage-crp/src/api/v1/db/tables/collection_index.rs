use std::sync::Arc;

use api_utils::ApiResult;
use axum::extract::State;

use crate::context::Context;

/// Get Collection Index Table
#[utoipa::path(
    get,
    path = "/v1/db/tables/collection-index",
    tag = "/v1/db/tables/collection-index",
    responses(
        (status = 200, description = "Get Collection Index Table", body = String)
    )
)]
pub async fn get_collection_index_table(State(ctx): State<Arc<Context>>) -> ApiResult<String> {
    let Context { db, .. } = &*ctx;

    let table = db.get_all_collection_entries_ascii_table()?;

    Ok(table)
}
