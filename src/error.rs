use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Missing Authorization Headers")]
    MissingAuthorization,
    #[error("Invalid Credentials")]
    InvalidCredentials,
    #[error("Asset does not exist")]
    AssetDoesNotExist,
    #[error("User does not exist")]
    UserDoesNotExist,
    #[error("This username is already registered")]
    UsernameTaken,
    #[error("Já existe um ativo com esse nome.")]
    AssetNameTaken,
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Template(#[from] askama::Error),
    #[error(transparent)]
    Jwt(#[from] jwt_simple::Error),
}

impl AppError {
    /// Converte o erro de uma escrita na tabela `assets`: a violação de `UNIQUE (name)`
    /// vira `AssetNameTaken` (409); os demais seguem como erro de banco.
    pub fn from_asset_write(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::Database(ref db_err) if db_err.is_unique_violation() => {
                Self::AssetNameTaken
            }
            err => Self::Database(err),
        }
    }
}

#[derive(Serialize)]
pub struct ErrorResponse {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let error_response = ErrorResponse {
            error: self.to_string(),
        };

        let status = match self {
            Self::UsernameTaken | Self::MissingAuthorization | Self::Validation(_) => {
                StatusCode::BAD_REQUEST
            }
            Self::AssetNameTaken => StatusCode::CONFLICT,
            Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::AssetDoesNotExist | Self::UserDoesNotExist => StatusCode::NOT_FOUND,
            Self::Database(_) | Self::Template(_) | Self::Jwt(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        (status, Json(error_response)).into_response()
    }
}
