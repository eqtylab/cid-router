use std::sync::Arc;

use api_utils::ApiResult;
use axum::extract::State;

use crate::context::Context;

/// Get Commit Table
#[utoipa::path(
    get,
    path = "/v1/db/tables/commit-table",
    tag = "/v1/db/tables/commit-table",
    responses(
        (status = 200, description = "Get Commit Table", body = String)
    )
)]
pub async fn get_commit_table(State(ctx): State<Arc<Context>>) -> ApiResult<String> {
    let Context { db, .. } = &*ctx;

    let table = db.get_all_repo_commits_ascii_table()?;

    Ok(table)
}
