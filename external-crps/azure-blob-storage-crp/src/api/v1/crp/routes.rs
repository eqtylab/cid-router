use std::sync::Arc;

use anyhow::Result;
use api_utils::ApiResult;
use axum::{
    extract::{Path, State},
    Json,
};
use routes::{AzureBlobStorageRouteMethod, IntoRoute};
use serde::Serialize;
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::{
    context::Context,
    db::{BlobId, BlobInfo},
};
#[derive(Serialize, ToSchema)]
pub struct CrpGetRoutesResponse {
    routes: Vec<Route>,
}

#[derive(Serialize, ToSchema)]
pub struct Route {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crp_id: Option<String>,
    #[serde(rename = "type")]
    pub type_: String,
    pub method: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

/// Get CID Routes
#[utoipa::path(
    get,
    path = "/v1/crp/routes/{cid}",
    tag = "/v1/crp/routes/{cid}",
    responses(
        (status = 200, description = "Get CID Routes", body = CrpGetRoutesResponse)
    )
)]
pub async fn get_routes(
    Path(cid): Path<String>,
    State(ctx): State<Arc<Context>>,
) -> ApiResult<Json<CrpGetRoutesResponse>> {
    let Context { db, .. } = &*ctx;

    let routes = db
        .get_blob_ids_and_infos_for_cid(cid)?
        .into_iter()
        .map(
            |(
                BlobId {
                    account,
                    container,
                    name,
                },
                BlobInfo {
                    timestamp,
                    size,
                    time_first_indexed,
                    time_last_checked,
                    ..
                },
            )| {
                let method = AzureBlobStorageRouteMethod {
                    account,
                    container,
                    name,
                };
                let metadata = json!({
                    "timestamp": timestamp,
                    "size": size,
                    "time_first_indexed": time_first_indexed,
                    "time_last_checked": time_last_checked,
                });

                Ok(method.into_route(None, Some(metadata))?)
            },
        )
        .collect::<Result<Vec<_>>>()?;
    let routes = routes.into_iter().map(Into::into).collect();

    Ok(Json(CrpGetRoutesResponse { routes }))
}

impl From<routes::Route> for Route {
    fn from(route: routes::Route) -> Self {
        let routes::Route {
            crp_id,
            type_,
            method,
            metadata,
        } = route;

        Self {
            crp_id,
            type_,
            method,
            metadata,
        }
    }
}
