//! Data/hora std-only do RHP (Fase A4): relogio do SO, espera e formato.
//!
//! Limites (documentados em `LANGUAGE_SPEC.md`):
//! - `now()`: segundos desde 1970-01-01T00:00:00Z (`Int`); relogio do
//!   sistema (sem monotonicidade garantida; pode andar para tras em
//!   ajuste de relogio).
//! - `sleep(s)`: bloqueia a thread atual (VM e single-thread); aceita
//!   `Int`/`Float` (fração = sub-segundo); negativo, NaN/infinito ou
//!   acima de 1 hora = erro (teto contra travamento acidental).
//! - `date_str(ts)`: `Int` (epoch) => `"AAAA-MM-DDTHH:MM:SSZ"` (UTC,
//!   sempre; sem timezones, sem horario de verao). Algoritmo civil
//!   proleptico proprio (dias desde a era civil), sem dependencias.

use super::type_name;
use super::VmError;
use rhp_bytecode::Value;
use std::time::{SystemTime, UNIX_EPOCH};

/// Teto de `sleep` em segundos (1 hora).
pub const MAX_SLEEP_SECS: f64 = 3600.0;

/// Segundos desde o epoch (UTC, relogio do sistema).
pub fn now_unix() -> Result<Value, VmError> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| VmError::new(format!("`now` com relogio anterior a 1970: {e}")))?
        .as_secs();
    i64::try_from(secs)
        .map(Value::Int)
        .map_err(|_| VmError::new("`now` fora do intervalo de `Int`".to_string()))
}

/// Bloqueia a thread por `secs` segundos; devolve `null`.
pub fn sleep_secs(value: &Value) -> Result<Value, VmError> {
    let secs = match value {
        Value::Int(v) => *v as f64,
        Value::Float(v) => *v,
        other => {
            return Err(VmError::new(format!(
                "`sleep` exige numero de segundos, encontrado {}",
                type_name(other)
            )));
        }
    };
    if !secs.is_finite() || secs < 0.0 {
        return Err(VmError::new(format!(
            "`sleep` exige segundos >= 0, encontrado {secs}"
        )));
    }
    if secs > MAX_SLEEP_SECS {
        return Err(VmError::new(format!(
            "`sleep` excedeu o teto ({MAX_SLEEP_SECS} segundos)"
        )));
    }
    std::thread::sleep(std::time::Duration::from_secs_f64(secs));
    Ok(Value::Null)
}

/// Formata epoch (`Int`) como `"AAAA-MM-DDTHH:MM:SSZ"` (UTC).
pub fn date_string(value: &Value) -> Result<Value, VmError> {
    let timestamp = match value {
        Value::Int(v) => *v,
        other => {
            return Err(VmError::new(format!(
                "`date_str` exige epoch (`Int`), encontrado {}",
                type_name(other)
            )));
        }
    };
    let days = timestamp.div_euclid(86_400);
    let secs_of_day = timestamp.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (
        secs_of_day / 3_600,
        (secs_of_day % 3_600) / 60,
        secs_of_day % 60,
    );
    Ok(Value::Str(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    )))
}

/// Dias desde 1970-01-01 => (ano, mes, dia) prolepticos (Hinnant:
/// desloca para a era civil que comeca em 0000-03-01).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era.div_euclid(1_460) + day_of_era.div_euclid(36_524)
        - day_of_era.div_euclid(146_096))
    .div_euclid(365);
    let base_year = year_of_era + era * 400;
    let day_of_year =
        day_of_era - (365 * year_of_era + year_of_era.div_euclid(4) - year_of_era.div_euclid(100));
    let month_prime = (5 * day_of_year + 2).div_euclid(153);
    let day = day_of_year - (153 * month_prime + 2).div_euclid(5) + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = if month <= 2 { base_year + 1 } else { base_year };
    (year, month, day)
}
