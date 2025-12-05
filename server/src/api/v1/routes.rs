use std::{collections::HashSet, str::FromStr, sync::Arc};

use api_utils::ApiResult;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use axum_extra::extract::TypedHeader;
use bytes::BytesMut;
use cid::Cid;
use cid_router_core::{
    cid::blake3_hash_to_cid,
    db::{Direction, OrderBy},
};
use futures::StreamExt;
use headers::{Authorization, ContentType};
use http_body::Frame;
use http_body_util::StreamBody;
use log::{error, info};
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
) -> ApiResult<Response> {
    // TODO - remove unwraps
    let cid = Cid::from_str(&cid).unwrap();
    let routes = ctx.core.db().routes_for_cid(cid).await.unwrap();
    let routes: Vec<cid_router_core::routes::Route> = routes.into_iter().collect();
    let token = auth.map(|TypedHeader(Authorization(bearer))| bearer.token().to_string());
    ctx.auth.service().await.authenticate(token).await?;

    for route in routes {
        // iterate through providers until you find a match on provider_id and provider_type
        let provider_id: String = route.provider_id.clone();
        if let Some(provider) = ctx
            .providers
            .iter()
            .find(|p| provider_id == p.provider_id() && route.provider_type == p.provider_type())
        {
            if let Some(route_resolver) = provider.capabilities().route_resolver {
                let stream = route_resolver.get_bytes(&route, None).await.unwrap();

                // Convert Stream<Item = Bytes> into a response body
                let body = StreamBody::new(
                    stream.map(|result| result.map(Frame::data).map_err(std::io::Error::other)),
                );

                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .body(Body::new(body))
                    .unwrap());
            }
        }
    }

    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap())
}

/// Create data for a CID
#[axum::debug_handler]
pub async fn create_data(
    auth: Option<TypedHeader<Authorization<headers::authorization::Bearer>>>,
    content_type: Option<TypedHeader<ContentType>>,
    State(ctx): State<Arc<Context>>,
    body: Body,
) -> ApiResult<Response> {
    let token = auth.map(|TypedHeader(Authorization(bearer))| bearer.token().to_string());
    ctx.auth.service().await.authenticate(token).await?;

    // Check if content-type is supported and translate to cid type
    let content_type = content_type.map(|TypedHeader(mime)| mime.to_string());
    let cid_type = match content_type.as_ref().map(|ct| ct.as_str()) {
        None => cid_router_core::cid::Codec::Raw,
        Some("application/x-www-form-urlencoded") => cid_router_core::cid::Codec::Raw,
        Some("application/octet-stream") => cid_router_core::cid::Codec::Raw,
        Some("application/vnd.ipld.dag-cbor") => cid_router_core::cid::Codec::DagCbor,
        _ => {
            return Ok(Response::builder()
                .status(StatusCode::UNSUPPORTED_MEDIA_TYPE)
                .body(Body::empty())?);
        }
    };

    // Read data - we assume this to be small enough to fit into memory for now
    let mut buffer = BytesMut::new();
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let Ok(chunk) = chunk else {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::empty())?);
        };
        buffer.extend_from_slice(&chunk);
    }

    // compute CID
    let data = buffer.freeze();
    let hash = blake3::hash(&data);
    let cid = blake3_hash_to_cid(hash.into(), cid_type);

    // Find writers
    let writers = ctx
        .providers
        .iter()
        .filter(|p| p.provider_is_eligible_for_cid(&cid))
        .filter_map(|p| p.capabilities().blob_writer.map(|w| (p, w)))
        .collect::<Vec<_>>();
    if writers.is_empty() {
        return Ok(Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body(Body::empty())?);
    }

    let existing = ctx.core.db().routes_for_cid(cid).await?;
    let existing_ids = existing
        .iter()
        .map(|r| r.provider_id.clone())
        .collect::<HashSet<_>>();

    let mut outcome = Vec::new();
    for (crp, writer) in writers {
        if existing_ids.contains(&crp.provider_id()) {
            error!(
                "Skipping put to provider {} as route already exists",
                crp.provider_id()
            );
            continue;
        }
        let data = data.clone();
        let res = writer.put_blob(None, &cid, &data).await;
        outcome.push((crp, res));
    }

    for (provider, res) in &outcome {
        if res.is_ok() {
            let route = cid_router_core::routes::Route::builder(*provider)
                .cid(cid)
                .multicodec(cid_router_core::cid::Codec::Raw)
                .size(data.len() as u64)
                .url(cid.to_string())
                .build(&ctx.core)?;
            ctx.core.db().insert_route(&route).await?;
        }
    }

    let json = serde_json::json!({
        "cid": cid.to_string(),
        "size": data.len(),
        "location": format!("/v1/data/{}", cid)
    });

    Ok(Response::builder()
        .status(StatusCode::CREATED)
        .header(header::LOCATION, format!("/v1/data/{}", cid))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json.to_string()))
        .unwrap())
}
