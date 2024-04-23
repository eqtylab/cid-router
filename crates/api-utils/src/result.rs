use crate::error::ApiError;

pub type ApiResult<T> = Result<T, ApiError>;
