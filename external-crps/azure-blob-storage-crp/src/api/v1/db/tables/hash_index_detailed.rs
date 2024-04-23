use std::sync::Arc;

use api_utils::ApiResult;
use axum::extract::State;

use crate::context::Context;

/// Get Hash Index Table
#[utoipa::path(
    get,
    path = "/v1/db/tables/hash-index-detailed",
    tag = "/v1/db/tables/hash-index-detailed",
    responses(
        (status = 200, description = "Get Hash Index Table", body = String)
    )
)]
pub async fn get_hash_index_detailed_table(State(ctx): State<Arc<Context>>) -> ApiResult<String> {
    let Context { db, .. } = &*ctx;

    let table = db.get_all_hash_entries_with_blob_info_ascii_table()?;

    Ok(table)
}
