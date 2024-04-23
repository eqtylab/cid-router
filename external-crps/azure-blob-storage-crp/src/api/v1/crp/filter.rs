use std::sync::Arc;

use api_utils::ApiResult;
use axum::{extract::State, Json};
use cid_filter::{
    table::{
        multicodec::{BLAKE3_HASHSEQ, RAW},
        multihash::BLAKE3,
    },
    CidFilter, CodeFilter,
};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

use crate::context::Context;
#[derive(Serialize, ToSchema)]
pub struct CrpGetFilterResponse {
    filter: Value,
}

/// Get CRP CID Filter
#[utoipa::path(
    get,
    path = "/v1/crp/filter",
    tag = "/v1/crp/filter",
    responses(
        (status = 200, description = "Get CRP CID Filter", body = CrpGetFilterResponse)
    )
)]
pub async fn get_filter(State(ctx): State<Arc<Context>>) -> ApiResult<Json<CrpGetFilterResponse>> {
    let _ = &*ctx;

    let filter = CidFilter::MultihashCodeFilter(CodeFilter::Eq(BLAKE3))
        & (CidFilter::CodecFilter(CodeFilter::Eq(RAW))
            | CidFilter::CodecFilter(CodeFilter::Eq(BLAKE3_HASHSEQ)));

    let filter = serde_json::to_value(filter)?;

    Ok(Json(CrpGetFilterResponse { filter }))
}
