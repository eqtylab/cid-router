pub mod v1;

use std::sync::Arc;

use anyhow::Result;
use axum::{response::Redirect, routing::get, Router};
use log::info;
use tokio::net::TcpListener;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::context::Context;

#[derive(OpenApi)]
#[openapi(
    paths(
        v1::crp::filter::get_filter,
        v1::crp::routes::get_routes,
        v1::db::tables::cid_lookup_table::get_cid_lookup_table,
        v1::db::tables::commit_table::get_commit_table,
        v1::status::get_status,
    ),
    components(
        schemas(
            v1::crp::filter::CrpGetFilterResponse,
            v1::crp::routes::CrpGetRoutesResponse,
            v1::crp::routes::Route,
            v1::status::StatusResponse,
        )
    ),
    tags(
        (name = "Github CRP", description = "Github CRP API")
    )
)]
struct ApiDoc;

pub async fn start(ctx: Arc<Context>) -> Result<()> {
    let Context { port, .. } = &*ctx;

    let addr = format!("0.0.0.0:{port}");

    info!("🚀 Starting Github CRP");
    info!("🚀 HTTP API = {addr}");

    let router = Router::new()
        .merge(
            SwaggerUi::new("/swagger")
                .config(utoipa_swagger_ui::Config::default().try_it_out_enabled(true))
                .url("/api-docs/openapi.json", ApiDoc::openapi()),
        )
        .route(
            "/",
            get(move || async move { Redirect::temporary("/swagger") }),
        )
        .route("/v1/crp/filter", get(v1::crp::filter::get_filter))
        .route("/v1/crp/routes/:cid", get(v1::crp::routes::get_routes))
        .route(
            "/v1/db/tables/cid-lookup-table",
            get(v1::db::tables::cid_lookup_table::get_cid_lookup_table),
        )
        .route(
            "/v1/db/tables/commit-table",
            get(v1::db::tables::commit_table::get_commit_table),
        )
        .route("/v1/status", get(v1::status::get_status))
        .with_state(ctx);

    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, router).await?;

    Ok(())
}
