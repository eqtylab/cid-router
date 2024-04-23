use std::sync::Arc;

use api_utils::ApiResult;
use axum::extract::State;

use crate::context::Context;

/// Get Blob Index Table
#[utoipa::path(
    get,
    path = "/v1/db/tables/blob-index",
    tag = "/v1/db/tables/blob-index",
    responses(
        (status = 200, description = "Get Blob Index Table", body = BlobIndexTableResponse)
    )
)]
pub async fn get_blob_index_table(State(ctx): State<Arc<Context>>) -> ApiResult<String> {
    let Context { db, .. } = &*ctx;

    let table = db.get_all_blob_entries_ascii_table()?;

    Ok(table)
}
