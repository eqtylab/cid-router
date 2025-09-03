use std::{str::FromStr, sync::Arc};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
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
        .filter(|provider| provider.provider_is_eligible_for_cid(&cid))
        .collect::<Vec<_>>();

    let provider_requests = eligible_providers
        .into_iter()
        .map(|provider| async move {
            if let Some(routes_resolver) = provider.capabilities().routes_resolver {
                match routes_resolver.get_routes(&cid).await {
                    Ok(routes) => routes,
                    Err(e) => {
                        log::error!(
                            "failed to get routes for cid={cid} from provider (TODO: provider info): {e}"
                        );
                        vec![]
                    }
                }
            } else {
                vec![]
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

impl From<cid_router_core::routes::Route> for Route {
    fn from(route: cid_router_core::routes::Route) -> Self {
        let cid_router_core::routes::Route {
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
