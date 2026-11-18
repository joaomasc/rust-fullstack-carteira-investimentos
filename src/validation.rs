use crate::error::AppError;

const MAX_ASSET_NAME_LEN: usize = 50;
const MIN_USERNAME_LEN: usize = 3;
const MAX_USERNAME_LEN: usize = 32;
const MIN_PASSWORD_LEN: usize = 6;

fn invalid(message: impl Into<String>) -> AppError {
    AppError::Validation(message.into())
}

/// Remove espaços das pontas e garante que o nome do ativo não é vazio nem longo demais.
pub fn asset_name(raw: &str) -> Result<String, AppError> {
    let name = raw.trim();

    if name.is_empty() {
        return Err(invalid("O nome do ativo é obrigatório."));
    }
    if name.chars().count() > MAX_ASSET_NAME_LEN {
        return Err(invalid(format!(
            "O nome do ativo pode ter no máximo {MAX_ASSET_NAME_LEN} caracteres."
        )));
    }

    Ok(name.to_string())
}

/// Aceita apenas números finitos maiores que zero (rejeita NaN, infinito, zero e negativos).
pub fn positive_number(value: f64, field: &str) -> Result<f64, AppError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(invalid(format!(
            "{field} deve ser um número maior que zero."
        )))
    }
}

/// Converte o texto de um formulário em número positivo. Aceita vírgula ou ponto decimal.
pub fn parse_positive_number(raw: &str, field: &str) -> Result<f64, AppError> {
    let normalized = raw.trim().replace(',', ".");
    let value: f64 = normalized
        .parse()
        .map_err(|_| invalid(format!("{field} deve ser um número válido.")))?;

    positive_number(value, field)
}

pub fn username(username: &str) -> Result<(), AppError> {
    let len = username.chars().count();

    if (MIN_USERNAME_LEN..=MAX_USERNAME_LEN).contains(&len) {
        Ok(())
    } else {
        Err(invalid(format!(
            "O usuário deve ter entre {MIN_USERNAME_LEN} e {MAX_USERNAME_LEN} caracteres."
        )))
    }
}

pub fn password(password: &str) -> Result<(), AppError> {
    if password.chars().count() >= MIN_PASSWORD_LEN {
        Ok(())
    } else {
        Err(invalid(format!(
            "A senha deve ter pelo menos {MIN_PASSWORD_LEN} caracteres."
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_name_is_trimmed() {
        assert_eq!(asset_name("  Bitcoin ").unwrap(), "Bitcoin");
    }

    #[test]
    fn asset_name_rejects_empty_and_too_long() {
        assert!(asset_name("   ").is_err());
        assert!(asset_name(&"a".repeat(MAX_ASSET_NAME_LEN + 1)).is_err());
        assert!(asset_name(&"a".repeat(MAX_ASSET_NAME_LEN)).is_ok());
    }

    #[test]
    fn positive_number_rejects_invalid_values() {
        assert!(positive_number(0.0, "x").is_err());
        assert!(positive_number(-1.0, "x").is_err());
        assert!(positive_number(f64::NAN, "x").is_err());
        assert!(positive_number(f64::INFINITY, "x").is_err());
        assert_eq!(positive_number(1.5, "x").unwrap(), 1.5);
    }

    #[test]
    fn parse_accepts_comma_and_dot() {
        assert_eq!(parse_positive_number("10,5", "x").unwrap(), 10.5);
        assert_eq!(parse_positive_number(" 10.5 ", "x").unwrap(), 10.5);
    }

    #[test]
    fn parse_rejects_text_and_non_positive() {
        assert!(parse_positive_number("abc", "x").is_err());
        assert!(parse_positive_number("", "x").is_err());
        assert!(parse_positive_number("0", "x").is_err());
        assert!(parse_positive_number("-3", "x").is_err());
        assert!(parse_positive_number("NaN", "x").is_err());
    }

    #[test]
    fn username_and_password_rules() {
        assert!(username("ab").is_err());
        assert!(username("ana").is_ok());
        assert!(username(&"a".repeat(MAX_USERNAME_LEN + 1)).is_err());
        assert!(password("12345").is_err());
        assert!(password("123456").is_ok());
    }
}
