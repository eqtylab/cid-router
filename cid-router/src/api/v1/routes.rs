use std::{collections::HashMap, str::FromStr, sync::Arc};

use api_utils::ApiResult;
use axum::{
    extract::{Path, State},
    Json,
};
use cid::Cid;
use futures::stream::StreamExt;
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

use crate::context::Context;

#[derive(Serialize, ToSchema)]
pub struct RoutesResponse {
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
    let Context { providers, .. } = &*ctx;

    let cid = Cid::from_str(&cid)?;

    let eligible_providers = providers
        .iter()
        .filter(|(_, provider)| provider.provider_is_eligible_for_cid(&cid))
        .collect::<HashMap<_, _>>();

    let provider_requests = eligible_providers
        .into_iter()
        .map(|(provider_id, provider)| async move {
            match provider.get_routes_for_cid(&cid).await {
                Ok(routes) => routes,
                Err(e) => {
                    log::error!(
                        "failed to get routes for cid={cid} from provider={provider_id}: {e}"
                    );
                    vec![]
                }
            }
        })
        .collect::<Vec<_>>();

    let routes = futures::stream::iter(provider_requests.into_iter())
        .buffered(5)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    let routes = routes.into_iter().map(Into::into).collect();

    Ok(Json(RoutesResponse { routes }))
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
