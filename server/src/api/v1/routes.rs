use std::{str::FromStr, sync::Arc};

use api_utils::ApiResult;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use axum_extra::extract::TypedHeader;
use cid::Cid;
use cid_router_core::{
    db::{Direction, OrderBy},
};
use headers::Authorization;
use log::info;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

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
    pub cid: String,
}

impl From<cid_router_core::routes::Route> for Route {
    fn from(route: cid_router_core::routes::Route) -> Self {
        let cid_router_core::routes::Route {
            provider_type,
            provider_id,
            size,
            url: route,
            cid,
            ..
        } = route;

        Self {
            provider_id,
            r#type: provider_type.to_string(),
            size,
            url: route,
            cid: cid.to_string(),
        }
    }
}

#[derive(Deserialize, IntoParams)]
pub struct ListRoutesQuery {
    direction: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
}
// List routes
#[utoipa::path(
    get,
    path = "/v1/routes",
    tag = "/v1/routes",
    params(
        ListRoutesQuery,
        ("authorization" = Option<String>, Header, description = "Bearer token for authentication")
    ),
    responses(
        (status = 200, description = "List routes", body = Vec<Route>)
    )
)]
pub async fn list_routes(
    State(ctx): State<Arc<Context>>,
    query: Query<ListRoutesQuery>,
    auth: Option<TypedHeader<Authorization<headers::authorization::Bearer>>>,
) -> ApiResult<Json<Vec<Route>>> {
    let token = auth.map(|TypedHeader(Authorization(bearer))| bearer.token().to_string());
    ctx.auth.service().await.authenticate(token).await?;

    let direction = query.0.direction.unwrap_or_else(|| "DESC".to_string());
    let offset = query.0.offset.unwrap_or(0);
    let limit = query.0.limit.unwrap_or(100);
    let direction = Direction::from_str(&direction).unwrap();

    let routes = ctx
        .core
        .db()
        .list_routes(OrderBy::CreatedAt(direction), offset, limit)
        .await?;
    let routes = routes.into_iter().map(Route::from).collect();

    Ok(Json(routes))
}

/// Get routes for a CID
#[utoipa::path(
    get,
    path = "/v1/routes/{cid}",
    tag = "/v1/routes/{cid}",
    params(
        ("authorization" = Option<String>, Header, description = "Bearer token for authentication")
    ),
    responses(
        (status = 200, description = "Get routes for a CID", body = RoutesResponse)
    )
)]
pub async fn get_routes(
    Path(cid): Path<String>,
    auth: Option<TypedHeader<Authorization<headers::authorization::Bearer>>>,
    State(ctx): State<Arc<Context>>,
) -> ApiResult<Json<RoutesResponse>> {
    let token = auth.map(|TypedHeader(Authorization(bearer))| bearer.token().to_string());
    ctx.auth.service().await.authenticate(token).await?;

    let cid = Cid::from_str(&cid)?;
    info!("finding routes for cid: {cid}");
    let routes = ctx.core.db().routes_for_cid(cid).await?;
    let routes = routes.into_iter().map(Route::from).collect();

    Ok(Json(RoutesResponse { routes }))
}
