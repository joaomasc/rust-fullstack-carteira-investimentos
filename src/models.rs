use serde::Serialize;
use sqlx::FromRow;

#[derive(Serialize, Clone)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub unit_value: f64,
}

/// Posição de uma pessoa usuária em um ativo (quantidade que ela possui).
#[derive(Serialize, Clone, FromRow)]
pub struct Holding {
    pub id: i64,
    pub asset_id: i64,
    pub asset_name: String,
    pub unit_value: f64,
    pub quantity: f64,
}

impl Holding {
    pub fn total_value(&self) -> f64 {
        self.unit_value * self.quantity
    }
}

/// Valor total da carteira: soma de `quantidade × valor unitário` de cada posição.
pub fn wallet_total(holdings: &[Holding]) -> f64 {
    holdings.iter().map(Holding::total_value).sum()
}

pub struct UserRecord {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holding(unit_value: f64, quantity: f64) -> Holding {
        Holding {
            id: 1,
            asset_id: 1,
            asset_name: "Teste".to_string(),
            unit_value,
            quantity,
        }
    }

    #[test]
    fn total_value_multiplies_quantity_by_unit_value() {
        assert_eq!(holding(10.0, 3.0).total_value(), 30.0);
    }

    #[test]
    fn wallet_total_sums_all_holdings() {
        let holdings = vec![holding(10.0, 5.0), holding(2.5, 4.0)];
        assert_eq!(wallet_total(&holdings), 60.0);
    }

    #[test]
    fn wallet_total_of_empty_wallet_is_zero() {
        assert_eq!(wallet_total(&[]), 0.0);
    }
}
