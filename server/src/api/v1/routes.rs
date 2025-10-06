use std::{str::FromStr, sync::Arc};

use api_utils::ApiResult;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use axum_extra::extract::TypedHeader;
use bytes::Bytes;
use cid::Cid;
use futures::StreamExt;
use headers::Authorization;
use http_body::Frame;
use http_body_util::StreamBody;
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

/// Get a data stream for a CID
#[utoipa::path(
    get,
    path = "/v1/data/{cid}",
    tag = "/v1/data/{cid}",
    params(
        ("authorization" = String, Header, description = "Bearer token for authentication")
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
    let routes: Vec<cid_router_core::routes::Route> = routes
        .into_iter()
        .map(cid_router_core::routes::Route::from)
        .collect();
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
                let body = StreamBody::new(stream.map(|result| {
                    result
                        .map(|bytes| Frame::data(bytes))
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                }));

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
