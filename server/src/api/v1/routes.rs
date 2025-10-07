use std::{str::FromStr, sync::Arc};

use api_utils::ApiResult;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use axum_extra::extract::TypedHeader;
use bytes::Bytes;
use cid::Cid;
use cid_router_core::db::{Direction, OrderBy};
use futures::StreamExt;
use headers::Authorization;
use http_body::Frame;
use http_body_util::StreamBody;
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
    ),
    responses(
        (status = 200, description = "List routes", body = Vec<Route>)
    )
)]
pub async fn list_routes(
    State(ctx): State<Arc<Context>>,
    query: Query<ListRoutesQuery>,
) -> ApiResult<Json<Vec<Route>>> {
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
    responses(
        (status = 200, description = "Get routes for a CID", body = RoutesResponse)
    )
)]
pub async fn get_routes(
    Path(cid): Path<String>,
    State(ctx): State<Arc<Context>>,
) -> ApiResult<Json<RoutesResponse>> {
    let cid = Cid::from_str(&cid)?;
    info!("finding routes for cid: {cid}");
    let routes = ctx.core.db().routes_for_cid(cid).await?;
    let routes = routes.into_iter().map(Route::from).collect();

    Ok(Json(RoutesResponse { routes }))
}

/// Get a data stream for a CID
#[utoipa::path(
    get,
    path = "/v1/data/{cid}",
    tag = "/v1/data/{cid}",
    params(
        ("authorization" = Option<String>, Header, description = "Bearer token for authentication")
    ),
    responses(
        (status = 200, description = "Get data for a CID", body = RoutesResponse)
    )
)]
pub async fn get_data(
    Path(cid): Path<String>,
    auth: Option<TypedHeader<Authorization<headers::authorization::Bearer>>>,
    State(ctx): State<Arc<Context>>,
) -> Response {
    // TODO - remove unwraps
    let cid = Cid::from_str(&cid).unwrap();
    let routes = ctx.core.db().routes_for_cid(cid).await.unwrap();
    let routes: Vec<cid_router_core::routes::Route> = routes.into_iter().collect();
    let token =
        auth.map(|TypedHeader(Authorization(bearer))| Bytes::from(bearer.token().to_string()));

    for route in routes {
        // iterate through providers until you find a match on provider_id and provider_type
        let provider_id: String = route.provider_id.clone();
        if let Some(provider) = ctx
            .providers
            .iter()
            .find(|p| provider_id == p.provider_id() && route.provider_type == p.provider_type())
        {
            if let Some(route_resolver) = provider.capabilities().route_resolver {
                let stream = route_resolver.get_bytes(&route, token).await.unwrap();

                // Convert Stream<Item = Bytes> into a response body
                let body = StreamBody::new(
                    stream.map(|result| result.map(Frame::data).map_err(std::io::Error::other)),
                );

                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .body(Body::new(body))
                    .unwrap();
            }
        }
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap()
}
