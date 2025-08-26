pub mod v1;

use std::sync::Arc;

use anyhow::Result;
use axum::{response::Redirect, routing::get, Router};
use log::info;
use routes;
use tokio::net::TcpListener;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::context::Context;

#[derive(OpenApi)]
#[openapi(
    paths(
        v1::providers::get_providers,
        v1::routes::get_routes,
        v1::status::get_status,
    ),
    components(
        schemas(
            v1::providers::ProvidersResponse,
            v1::routes::RoutesResponse,
            v1::routes::Route,
            v1::status::StatusResponse,
            routes::AzureBlobStorageRouteMethod,
            routes::UrlRouteMethod,
            routes::IpfsRouteMethod,
            routes::IrohRouteMethod,
            routes::AwsS3RouteMethod,
        )
    ),
    tags(
        (name = "CID Router", description = "CID Router API")
    )
)]
struct ApiDoc;

pub async fn start(ctx: Arc<Context>) -> Result<()> {
    let Context { port, .. } = &*ctx;

    let addr = format!("0.0.0.0:{port}");

    info!("🚀 Starting CID Router");
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
        .route("/v1/providers", get(v1::providers::get_providers))
        .route("/v1/routes/:cid", get(v1::routes::get_routes))
        .route("/v1/status", get(v1::status::get_status))
        .with_state(ctx);

    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, router).await?;

    Ok(())
}

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
