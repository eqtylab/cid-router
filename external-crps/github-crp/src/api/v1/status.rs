use std::sync::Arc;

use api_utils::ApiResult;
use axum::{extract::State, Json};
use serde::Serialize;
use utoipa::ToSchema;

use crate::context::Context;

#[derive(Serialize, ToSchema)]
pub struct StatusResponse {
    uptime: i64,
}

/// Get providers
#[utoipa::path(
    get,
    path = "/v1/status",
    tag = "/v1/status",
    responses(
        (status = 200, description = "Get status", body = StatusResponse)
    )
)]
pub async fn get_status(State(ctx): State<Arc<Context>>) -> ApiResult<Json<StatusResponse>> {
    let Context { start_time, .. } = &*ctx;

    let uptime = chrono::Utc::now().timestamp() - *start_time;

    Ok(Json(StatusResponse { uptime }))
}
