use std::{str::FromStr, sync::Arc};

use anyhow::Result;
use api_utils::ApiResult;
use axum::{
    extract::{Path, State},
    Json,
};
use cid::Cid;
use routes::{GithubRef, GithubRouteMethod, IntoRoute};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

use crate::{context::Context, db::RepoId};
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

    let cid = Cid::from_str(&cid)?;

    let commit = hex::encode(cid.hash().digest());

    let routes = db
        .get_repos_with_commits_for_cid(&cid)?
        .into_iter()
        .map(|RepoId { owner, repo }| {
            Ok(GithubRouteMethod {
                owner,
                repo,
                ref_: GithubRef::Commit(commit.clone()),
                path: None,
            }
            .into_route(None)?)
        })
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
