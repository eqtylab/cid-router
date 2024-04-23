use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug)]
pub struct ApiError {
    status_code: StatusCode,
    body: ApiErrorBody,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ApiErrorBody {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    callstack: Option<Callstack>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Callstack {
    Internal(String),
    External {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<Value>,
    },
}

impl ApiError {
    pub fn new(status_code: StatusCode, error: impl Into<String>) -> Self {
        let error = error.into();
        let callstack = None;

        Self {
            status_code,
            body: ApiErrorBody { error, callstack },
        }
    }

    pub fn new_with_external_error(
        status_code: StatusCode,
        error: impl Into<String>,
        url: impl Into<String>,
        external_error: Option<Value>,
    ) -> Self {
        let error = error.into();
        let url = url.into();
        let callstack = Some(Callstack::External {
            url,
            error: external_error,
        });

        Self {
            status_code,
            body: ApiErrorBody { error, callstack },
        }
    }
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        let err = err.into();
        let status_code = StatusCode::INTERNAL_SERVER_ERROR;
        let error = err.to_string();
        let callstack = Some(Callstack::Internal(err.backtrace().to_string()));

        Self {
            status_code,
            body: ApiErrorBody { error, callstack },
        }
    }
}

impl std::fmt::Debug for Callstack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Callstack::Internal(inner) => write!(f, "{}", inner),
            Callstack::External { url, error } => {
                write!(f, "url: {}\n error: {:?}", url, error)
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        log::error!("API error: {:#?}", self);

        let Self { status_code, body } = self;

        (
            status_code,
            serde_json::to_string(&body).unwrap_or("Unrepresentable error.".to_owned()),
        )
            .into_response()
    }
}
