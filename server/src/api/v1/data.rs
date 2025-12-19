use std::{collections::HashSet, str::FromStr, sync::Arc};

use api_utils::{ApiError, ApiResult};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use axum_extra::extract::TypedHeader;
use bytes::BytesMut;
use cid::Cid;
use cid_router_core::cid::{blake3_hash_to_cid, Codec};
use futures::StreamExt;
use headers::{Authorization, ContentType};
use http_body::Frame;
use http_body_util::StreamBody;
use log::info;
use serde::Serialize;
use utoipa::ToSchema;

use crate::context::Context;

/// Get a data stream for a CID
#[utoipa::path(
    get,
    path = "/v1/data/{cid}",
    tag = "/v1/data/{cid}",
    params(
        ("authorization" = Option<String>, Header, description = "Bearer token for authentication")
    ),
    responses(
        (status = 200, description = "Get raw data for a CID", content_type = "application/octet-stream"),
        (status = 404, description = "No route found for CID")
    )
)]
pub async fn get_data(
    Path(cid): Path<String>,
    auth: Option<TypedHeader<Authorization<headers::authorization::Bearer>>>,
    State(ctx): State<Arc<Context>>,
) -> ApiResult<Response> {
    let cid =
        Cid::from_str(&cid).map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e.to_string()))?;
    let routes = ctx.core.db().routes_for_cid(cid).await.map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch routes for cid {}: {}", cid, e),
        )
    })?;
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
                let stream = route_resolver.get_bytes(&route, None).await.map_err(|e| {
                    ApiError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!(
                            "Failed to get bytes for cid {} from provider {}: {}",
                            cid, provider_id, e
                        ),
                    )
                })?;

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

    Err(ApiError::new(
        StatusCode::NOT_FOUND,
        "No route found for CID",
    ))
}

#[derive(Serialize, ToSchema)]
pub struct CreateDataResponse {
    pub cid: String,
    pub size: usize,
    pub location: String,
}

/// Create data for a CID
#[utoipa::path(
    post,
    path = "/v1/data",
    tag = "/v1/data",
    params(
        ("authorization" = Option<String>, Header, description = "Bearer token for authentication")
    ),
    responses(
        (status = 200, description = "Get data for a CID", body = CreateDataResponse),
        (status = 415, description = "Unsupported content-type"),
        (status = 503, description = "No eligible writers found for CID"),
    )
)]
#[axum::debug_handler]
pub async fn create_data(
    auth: Option<TypedHeader<Authorization<headers::authorization::Bearer>>>,
    content_type: Option<TypedHeader<ContentType>>,
    State(ctx): State<Arc<Context>>,
    body: Body,
) -> ApiResult<Json<CreateDataResponse>> {
    let token = auth.map(|TypedHeader(Authorization(bearer))| bearer.token().to_string());
    ctx.auth.service().await.authenticate(token).await?;

    // Check if content-type is supported and translate to cid type
    let content_type = content_type.map(|TypedHeader(mime)| mime.to_string());
    let cid_type = match content_type.as_ref().map(|ct| ct.as_str()) {
        None => Codec::Raw,
        Some("application/x-www-form-urlencoded") => Codec::Raw,
        Some("application/octet-stream") => Codec::Raw,
        Some("application/vnd.ipld.dag-cbor") => Codec::DagCbor,
        _ => {
            return Err(ApiError::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Unsupported content-type",
            ))
        }
    };

    // Read data - we assume this to be small enough to fit into memory for now
    let mut buffer = BytesMut::new();
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let Ok(chunk) = chunk else {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Failed to read request body",
            ));
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
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "No eligible writers found for CID",
        ));
    }

    let existing = ctx.core.db().routes_for_cid(cid).await?;
    let existing_ids = existing
        .iter()
        .map(|r| r.provider_id.clone())
        .collect::<HashSet<_>>();

    let mut outcome = Vec::new();
    for (crp, writer) in writers {
        if existing_ids.contains(&crp.provider_id()) {
            info!(
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

    Ok(Json(CreateDataResponse {
        cid: cid.to_string(),
        size: data.len(),
        location: format!("/v1/data/{}", cid),
    }))
}
