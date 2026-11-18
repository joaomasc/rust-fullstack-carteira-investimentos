use std::convert::Infallible;

use axum::extract::FromRequestParts;
use sqlx::PgPool;

use crate::{
    app::AppState,
    models::{Asset, Holding, UserRecord},
};

pub struct Repository {
    db: PgPool,
}

impl Repository {
    pub async fn list_assets(&self) -> sqlx::Result<Vec<Asset>> {
        sqlx::query_as!(
            Asset,
            "SELECT id, name, unit_value
             FROM assets;"
        )
        .fetch_all(&self.db)
        .await
    }

    pub async fn create_asset(&self, name: String, unit_value: f64) -> sqlx::Result<Asset> {
        sqlx::query_as!(
            Asset,
            "INSERT INTO assets (name, unit_value)
             VALUES ($1, $2)
             RETURNING id, name, unit_value;",
            name,
            unit_value
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn update_asset(
        &self,
        asset_id: i64,
        name: Option<String>,
        unit_value: Option<f64>,
    ) -> sqlx::Result<Option<Asset>> {
        sqlx::query_as!(
            Asset,
            "UPDATE assets
             SET name=COALESCE($2, name),
                 unit_value=COALESCE($3, unit_value)
             WHERE id=$1
             RETURNING id, name, unit_value;",
            asset_id,
            name,
            unit_value
        )
        .fetch_optional(&self.db)
        .await
    }

    pub async fn list_holdings(&self, user_id: i64) -> sqlx::Result<Vec<Holding>> {
        sqlx::query_as::<_, Holding>(
            "SELECT h.id, h.asset_id, a.name AS asset_name, a.unit_value, h.quantity
             FROM holdings h
             JOIN assets a ON a.id = h.asset_id
             WHERE h.user_id = $1
             ORDER BY a.name;",
        )
        .bind(user_id)
        .fetch_all(&self.db)
        .await
    }

    /// Cria a posição ou, se a pessoa já possui o ativo, substitui a quantidade.
    pub async fn set_holding(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: f64,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO holdings (user_id, asset_id, quantity)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_id, asset_id)
             DO UPDATE SET quantity = EXCLUDED.quantity;",
        )
        .bind(user_id)
        .bind(asset_id)
        .bind(quantity)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Remove a posição apenas se ela pertencer à pessoa usuária. Retorna se algo foi removido.
    pub async fn delete_holding(&self, user_id: i64, holding_id: i64) -> sqlx::Result<bool> {
        let result = sqlx::query("DELETE FROM holdings WHERE id = $1 AND user_id = $2;")
            .bind(holding_id)
            .bind(user_id)
            .execute(&self.db)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn add_user(&self, username: &str, password_hash: &str) -> sqlx::Result<UserRecord> {
        sqlx::query_as!(
            UserRecord,
            "INSERT INTO users (username, password_hash)
             VALUES ($1, $2)
             RETURNING id, username, password_hash;",
            username,
            password_hash,
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn get_user_by_name(&self, username: &str) -> sqlx::Result<Option<UserRecord>> {
        sqlx::query_as!(
            UserRecord,
            "SELECT id, username, password_hash
             FROM users
             WHERE username = $1;",
            username
        )
        .fetch_optional(&self.db)
        .await
    }
}

impl FromRequestParts<AppState> for Repository {
    type Rejection = Infallible;

    async fn from_request_parts(
        _parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self {
            db: state.db.clone(),
        })
    }
}

#[cfg(test)]
impl From<PgPool> for Repository {
    fn from(db: PgPool) -> Self {
        Self { db }
    }
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;
    use crate::models::wallet_total;

    #[sqlx::test]
    async fn holdings_can_be_created_updated_listed_and_deleted(db: PgPool) {
        let repository = Repository::from(db);
        let user = repository.add_user("ana", "hash").await.unwrap();
        let bitcoin = repository
            .create_asset("Bitcoin".into(), 10.0)
            .await
            .unwrap();
        let ethereum = repository
            .create_asset("Ethereum".into(), 2.5)
            .await
            .unwrap();

        repository
            .set_holding(user.id, bitcoin.id, 3.0)
            .await
            .unwrap();
        repository
            .set_holding(user.id, ethereum.id, 4.0)
            .await
            .unwrap();
        // Definir de novo o mesmo ativo atualiza a quantidade em vez de duplicar.
        repository
            .set_holding(user.id, bitcoin.id, 5.0)
            .await
            .unwrap();

        let holdings = repository.list_holdings(user.id).await.unwrap();
        assert_eq!(holdings.len(), 2);
        assert_eq!(holdings[0].asset_name, "Bitcoin");
        assert_eq!(holdings[0].quantity, 5.0);
        assert_eq!(wallet_total(&holdings), 5.0 * 10.0 + 4.0 * 2.5);

        assert!(
            repository
                .delete_holding(user.id, holdings[0].id)
                .await
                .unwrap()
        );
        assert!(
            !repository
                .delete_holding(user.id, holdings[0].id)
                .await
                .unwrap()
        );
        assert_eq!(repository.list_holdings(user.id).await.unwrap().len(), 1);
    }

    #[sqlx::test]
    async fn holdings_are_private_to_each_user(db: PgPool) {
        let repository = Repository::from(db);
        let ana = repository.add_user("ana", "hash").await.unwrap();
        let bia = repository.add_user("bia", "hash").await.unwrap();
        let bitcoin = repository
            .create_asset("Bitcoin".into(), 10.0)
            .await
            .unwrap();

        repository
            .set_holding(ana.id, bitcoin.id, 1.0)
            .await
            .unwrap();
        let holdings = repository.list_holdings(ana.id).await.unwrap();

        assert!(repository.list_holdings(bia.id).await.unwrap().is_empty());
        assert!(
            !repository
                .delete_holding(bia.id, holdings[0].id)
                .await
                .unwrap()
        );
        assert_eq!(repository.list_holdings(ana.id).await.unwrap().len(), 1);
    }

    #[sqlx::test]
    async fn holding_for_unknown_asset_is_rejected(db: PgPool) {
        let repository = Repository::from(db);
        let user = repository.add_user("ana", "hash").await.unwrap();

        let result = repository.set_holding(user.id, 999, 1.0).await;
        assert!(matches!(
            result,
            Err(sqlx::Error::Database(ref e)) if e.is_foreign_key_violation()
        ));
    }
}
