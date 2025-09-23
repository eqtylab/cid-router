use std::{str::FromStr, sync::Arc};

use api_utils::ApiResult;
use axum::{
    extract::{Path, State},
    Json,
};
use cid::Cid;
use serde::Serialize;
use utoipa::ToSchema;

use crate::context::Context;

#[derive(Serialize, ToSchema)]
pub struct RoutesResponse {
    routes: Vec<Route>,
}

#[derive(Serialize, ToSchema)]
pub struct Route {
    pub provider_id: String,
    pub r#type: String,
    pub size: u64,
    pub url: String,
}

impl From<cid_router_core::routes::Route> for Route {
    fn from(route: cid_router_core::routes::Route) -> Self {
        let cid_router_core::routes::Route {
            provider_type,
            provider_id,
            size,
            route,
            ..
        } = route;

        Self {
            provider_id,
            r#type: provider_type.to_string(),
            size,
            url: route,
        }
    }
}

/// Get routes for a CID
#[utoipa::path(
    get,
    path = "/v1/routes/{cid}",
    tag = "/v1/routes/{cid}",
    responses(
        (status = 200, description = "Get routes for a CID", body = RoutesResponse)
    )
)]
pub async fn get_routes(
    Path(cid): Path<String>,
    State(ctx): State<Arc<Context>>,
) -> ApiResult<Json<RoutesResponse>> {
    let cid = Cid::from_str(&cid)?;
    let routes = ctx.core.db().routes_for_cid(cid).await?;
    let routes = routes.into_iter().map(Route::from).collect();

    Ok(Json(RoutesResponse { routes }))
}
