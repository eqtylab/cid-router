use std::sync::Arc;

use anyhow::Result;
use api_utils::ApiResult;
use axum::{
    extract::{Path, State},
    Json,
};
use routes::{AzureBlobStorageRouteMethod, IntoRoute};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

use crate::{context::Context, db::BlobId};
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
        .get_blob_ids_for_cid(cid)?
        .into_iter()
        .map(
            |BlobId {
                 account,
                 container,
                 name,
             }| {
                Ok(AzureBlobStorageRouteMethod {
                    account,
                    container,
                    name,
                }
                .into_route(None)?)
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
        } = route;

        Self {
            crp_id,
            type_,
            method,
        }
    }
}
