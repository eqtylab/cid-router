use std::{collections::HashMap, sync::Arc};

use api_utils::ApiResult;
use axum::{extract::State, Json};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

use crate::context::Context;

#[derive(Serialize, ToSchema)]
pub struct ProvidersResponse {
    providers: HashMap<String, Value>,
}

/// Get providers
#[utoipa::path(
    get,
    path = "/v1/providers",
    tag = "/v1/providers",
    responses(
        (status = 200, description = "Get providers", body = ProvidersResponse)
    )
)]
pub async fn get_providers(State(ctx): State<Arc<Context>>) -> ApiResult<Json<ProvidersResponse>> {
    let Context { providers, .. } = &*ctx;

    let providers = providers
        .iter()
        .map(|(id, provider)| (id.to_owned(), provider.provider_config()))
        .collect();

    Ok(Json(ProvidersResponse { providers }))
}
