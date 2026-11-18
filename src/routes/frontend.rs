use askama::Template;
use axum::{
    Form, Router,
    extract::Path,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::{
    CookieJar,
    cookie::{Cookie, SameSite},
};
use serde::Deserialize;

use crate::{
    app::AppState,
    auth::user::{UnauthenticatedUser, User},
    error::AppError,
    models::wallet_total,
    repository::Repository,
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/login", get(login_page).post(login))
        .route("/logout", post(logout))
        .route("/assets", post(create_asset))
        .route("/assets/{id}/update", post(update_asset))
        .route("/holdings", post(save_holding))
        .route("/holdings/{id}/delete", post(delete_holding))
}

// ---------------------------------------------------------------------------
// Formatação
// ---------------------------------------------------------------------------

fn format_money(value: f64) -> String {
    format!("R$ {value:.2}")
}

/// Formata um número sem zeros à direita (10.50000000 -> "10.5", 10.0 -> "10").
fn format_number(value: f64) -> String {
    format!("{value:.8}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

// ---------------------------------------------------------------------------
// Login
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage {
    error: Option<String>,
}

async fn login_page() -> Result<Html<String>, AppError> {
    let html = LoginPage { error: None }.render()?;
    Ok(Html(html))
}

fn login_error(status: StatusCode, message: String) -> Result<Response, AppError> {
    let html = LoginPage {
        error: Some(message),
    }
    .render()?;

    Ok((status, Html(html)).into_response())
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

async fn login(
    repository: Repository,
    jar: CookieJar,
    Form(request): Form<LoginForm>,
) -> Result<Response, AppError> {
    let username = request.username.trim().to_string();
    let password = request.password;

    let unauth_user = UnauthenticatedUser::new(username.clone(), password.clone());
    let user = match unauth_user.authenticate(&repository).await {
        Ok(user) => user,
        Err(AppError::UserDoesNotExist) => {
            // Primeiro acesso: a conta é criada, então valida usuário e senha antes.
            if let Err(err) =
                validation::username(&username).and_then(|_| validation::password(&password))
            {
                return login_error(StatusCode::BAD_REQUEST, err.to_string());
            }
            unauth_user.register(&repository).await?
        }
        Err(AppError::InvalidCredentials) => {
            return login_error(
                StatusCode::UNAUTHORIZED,
                "Usuário ou senha inválidos.".to_string(),
            );
        }
        Err(other_err) => return Err(other_err),
    };

    let token = user.auth_token()?;
    let cookie = Cookie::build(("token", token))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/");

    Ok((jar.add(cookie), Redirect::to("/")).into_response())
}

async fn logout(jar: CookieJar) -> impl IntoResponse {
    let cookie = Cookie::build(("token", "")).path("/");
    (jar.remove(cookie), Redirect::to("/login"))
}

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------

struct HoldingRow {
    id: i64,
    asset_id: i64,
    asset_name: String,
    unit_value: String,
    quantity: String,
    total: String,
}

struct AssetRow {
    id: i64,
    name: String,
    unit_value: String,
    unit_value_display: String,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardPage {
    username: String,
    total: String,
    holdings: Vec<HoldingRow>,
    assets: Vec<AssetRow>,
    error: Option<String>,
}

async fn dashboard(
    repository: &Repository,
    user: &User,
    status: StatusCode,
    error: Option<String>,
) -> Result<Response, AppError> {
    let holdings = repository.list_holdings(user.id()).await?;
    let mut assets = repository.list_assets().await?;
    assets.sort_by_key(|asset| asset.name.to_lowercase());

    let total = format_money(wallet_total(&holdings));

    let holdings = holdings
        .iter()
        .map(|holding| HoldingRow {
            id: holding.id,
            asset_id: holding.asset_id,
            asset_name: holding.asset_name.clone(),
            unit_value: format_money(holding.unit_value),
            quantity: format_number(holding.quantity),
            total: format_money(holding.total_value()),
        })
        .collect();

    let assets = assets
        .into_iter()
        .map(|asset| AssetRow {
            id: asset.id,
            name: asset.name,
            unit_value: format_number(asset.unit_value),
            unit_value_display: format_money(asset.unit_value),
        })
        .collect();

    let html = DashboardPage {
        username: user.username().clone(),
        total,
        holdings,
        assets,
        error,
    }
    .render()?;

    Ok((status, Html(html)).into_response())
}

/// Conclui uma ação de formulário: redireciona se deu certo, ou volta ao dashboard
/// mostrando a mensagem quando o erro é de validação. Outros erros seguem como estão.
async fn finish(
    repository: &Repository,
    user: &User,
    result: Result<(), AppError>,
) -> Result<Response, AppError> {
    match result {
        Ok(()) => Ok(Redirect::to("/").into_response()),
        Err(
            err
            @ (AppError::Validation(_) | AppError::AssetNameTaken | AppError::AssetDoesNotExist),
        ) => {
            dashboard(
                repository,
                user,
                StatusCode::BAD_REQUEST,
                Some(err.to_string()),
            )
            .await
        }
        Err(other_err) => Err(other_err),
    }
}

async fn index(repository: Repository, maybe_user: Option<User>) -> Result<Response, AppError> {
    let Some(user) = maybe_user else {
        return Ok(Redirect::to("/login").into_response());
    };

    dashboard(&repository, &user, StatusCode::OK, None).await
}

// ---------------------------------------------------------------------------
// Ativos
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct NewAssetForm {
    name: String,
    unit_value: String,
}

async fn try_create_asset(repository: &Repository, form: &NewAssetForm) -> Result<(), AppError> {
    let name = validation::asset_name(&form.name)?;
    let unit_value = validation::parse_positive_number(&form.unit_value, "O valor unitário")?;

    repository
        .create_asset(name, unit_value)
        .await
        .map_err(AppError::from_asset_write)?;

    Ok(())
}

async fn create_asset(
    repository: Repository,
    maybe_user: Option<User>,
    Form(form): Form<NewAssetForm>,
) -> Result<Response, AppError> {
    let Some(user) = maybe_user else {
        return Ok(Redirect::to("/login").into_response());
    };

    let result = try_create_asset(&repository, &form).await;
    finish(&repository, &user, result).await
}

#[derive(Deserialize)]
struct UpdateAssetForm {
    unit_value: String,
}

async fn try_update_asset(
    repository: &Repository,
    id: i64,
    form: &UpdateAssetForm,
) -> Result<(), AppError> {
    let unit_value = validation::parse_positive_number(&form.unit_value, "O valor unitário")?;

    match repository.update_asset(id, None, Some(unit_value)).await? {
        Some(_) => Ok(()),
        None => Err(AppError::AssetDoesNotExist),
    }
}

async fn update_asset(
    repository: Repository,
    maybe_user: Option<User>,
    Path(id): Path<i64>,
    Form(form): Form<UpdateAssetForm>,
) -> Result<Response, AppError> {
    let Some(user) = maybe_user else {
        return Ok(Redirect::to("/login").into_response());
    };

    let result = try_update_asset(&repository, id, &form).await;
    finish(&repository, &user, result).await
}

// ---------------------------------------------------------------------------
// Posições da carteira
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct HoldingForm {
    asset_id: i64,
    quantity: String,
}

async fn try_save_holding(
    repository: &Repository,
    user: &User,
    form: &HoldingForm,
) -> Result<(), AppError> {
    let quantity = validation::parse_positive_number(&form.quantity, "A quantidade")?;

    match repository
        .set_holding(user.id(), form.asset_id, quantity)
        .await
    {
        Ok(()) => Ok(()),
        Err(sqlx::Error::Database(db_err)) if db_err.is_foreign_key_violation() => {
            Err(AppError::AssetDoesNotExist)
        }
        Err(err) => Err(AppError::Database(err)),
    }
}

async fn save_holding(
    repository: Repository,
    maybe_user: Option<User>,
    Form(form): Form<HoldingForm>,
) -> Result<Response, AppError> {
    let Some(user) = maybe_user else {
        return Ok(Redirect::to("/login").into_response());
    };

    let result = try_save_holding(&repository, &user, &form).await;
    finish(&repository, &user, result).await
}

async fn delete_holding(
    repository: Repository,
    maybe_user: Option<User>,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    let Some(user) = maybe_user else {
        return Ok(Redirect::to("/login").into_response());
    };

    repository.delete_holding(user.id(), id).await?;
    Ok(Redirect::to("/").into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_money_uses_two_decimals() {
        assert_eq!(format_money(1234.5), "R$ 1234.50");
        assert_eq!(format_money(0.0), "R$ 0.00");
    }

    #[test]
    fn format_number_drops_trailing_zeros() {
        assert_eq!(format_number(10.0), "10");
        assert_eq!(format_number(10.5), "10.5");
        assert_eq!(format_number(0.00012), "0.00012");
        assert_eq!(format_number(100.0), "100");
        assert_eq!(format_number(0.0), "0");
    }
}
