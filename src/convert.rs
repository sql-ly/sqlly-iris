#[cfg(any(
    feature = "bigdecimal",
    feature = "chrono",
    feature = "rust_decimal",
    feature = "time",
    feature = "uuid"
))]
use crate::{Error, Result, Value};

#[cfg(feature = "uuid")]
impl From<uuid::Uuid> for Value {
    fn from(value: uuid::Uuid) -> Self {
        Self::String(value.to_string())
    }
}

#[cfg(feature = "uuid")]
impl TryFrom<&Value> for uuid::Uuid {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::String(value) => Self::parse_str(value)
                .map_err(|error| Error::Protocol(format!("invalid UUID {value:?}: {error}"))),
            other => Err(Error::Protocol(format!("cannot convert {other:?} to UUID"))),
        }
    }
}

#[cfg(feature = "rust_decimal")]
impl From<rust_decimal::Decimal> for Value {
    fn from(value: rust_decimal::Decimal) -> Self {
        Self::Decimal(value.to_string())
    }
}

#[cfg(feature = "rust_decimal")]
impl TryFrom<&Value> for rust_decimal::Decimal {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::Decimal(value) => value
                .parse()
                .map_err(|error| Error::Protocol(format!("invalid decimal {value:?}: {error}"))),
            other => Err(Error::Protocol(format!(
                "cannot convert {other:?} to rust_decimal::Decimal"
            ))),
        }
    }
}

#[cfg(feature = "bigdecimal")]
impl From<bigdecimal::BigDecimal> for Value {
    fn from(value: bigdecimal::BigDecimal) -> Self {
        Self::Decimal(value.to_string())
    }
}

#[cfg(feature = "bigdecimal")]
impl TryFrom<&Value> for bigdecimal::BigDecimal {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::Decimal(value) => value
                .parse()
                .map_err(|error| Error::Protocol(format!("invalid decimal {value:?}: {error}"))),
            other => Err(Error::Protocol(format!(
                "cannot convert {other:?} to bigdecimal::BigDecimal"
            ))),
        }
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveDateTime> for Value {
    fn from(value: chrono::NaiveDateTime) -> Self {
        Self::Timestamp(value.format("%Y-%m-%d %H:%M:%S%.f").to_string())
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::DateTime<chrono::Utc>> for Value {
    fn from(value: chrono::DateTime<chrono::Utc>) -> Self {
        Self::PosixTimestamp {
            seconds: value.timestamp(),
            micros: value.timestamp_subsec_micros(),
        }
    }
}

#[cfg(feature = "chrono")]
impl TryFrom<&Value> for chrono::NaiveDateTime {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::Timestamp(value) => Self::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
                .map_err(|error| Error::Protocol(format!("invalid timestamp {value:?}: {error}"))),
            Value::Date(value) => {
                Self::parse_from_str(&format!("{value} 00:00:00"), "%Y-%m-%d %H:%M:%S")
                    .map_err(|error| Error::Protocol(format!("invalid date {value:?}: {error}")))
            }
            Value::PosixTimestamp { seconds, micros } => {
                chrono::DateTime::from_timestamp(*seconds, micros * 1_000)
                    .map(|value| value.naive_utc())
                    .ok_or_else(|| {
                        Error::Protocol("POSIX timestamp is outside chrono's range".into())
                    })
            }
            other => Err(Error::Protocol(format!(
                "cannot convert {other:?} to chrono::NaiveDateTime"
            ))),
        }
    }
}

#[cfg(feature = "chrono")]
impl TryFrom<&Value> for chrono::DateTime<chrono::Utc> {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::PosixTimestamp { seconds, micros } => {
                Self::from_timestamp(*seconds, micros * 1_000).ok_or_else(|| {
                    Error::Protocol("POSIX timestamp is outside chrono's range".into())
                })
            }
            Value::Timestamp(value) => {
                chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
                    .map(|value| value.and_utc())
                    .map_err(|error| {
                        Error::Protocol(format!("invalid timestamp {value:?}: {error}"))
                    })
            }
            other => Err(Error::Protocol(format!(
                "cannot convert {other:?} to chrono::DateTime<Utc>"
            ))),
        }
    }
}

#[cfg(feature = "time")]
impl From<time::PrimitiveDateTime> for Value {
    fn from(value: time::PrimitiveDateTime) -> Self {
        Self::Timestamp(
            value
                .assume_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .expect("UTC date/time formatting is infallible")
                .replace('T', " ")
                .trim_end_matches('Z')
                .to_string(),
        )
    }
}

#[cfg(feature = "time")]
impl From<time::OffsetDateTime> for Value {
    fn from(value: time::OffsetDateTime) -> Self {
        Self::PosixTimestamp {
            seconds: value.unix_timestamp(),
            micros: value.nanosecond() / 1_000,
        }
    }
}

#[cfg(feature = "time")]
impl TryFrom<&Value> for time::PrimitiveDateTime {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::Timestamp(value) => parse_time_primitive(value),
            Value::PosixTimestamp { seconds, micros } => {
                let offset = time::OffsetDateTime::from_unix_timestamp(*seconds)
                    .map_err(|error| Error::Protocol(format!("invalid POSIX timestamp: {error}")))?
                    .replace_nanosecond(micros * 1_000)
                    .expect("microseconds are bounded by the protocol");
                Ok(time::PrimitiveDateTime::new(offset.date(), offset.time()))
            }
            other => Err(Error::Protocol(format!(
                "cannot convert {other:?} to time::PrimitiveDateTime"
            ))),
        }
    }
}

#[cfg(feature = "time")]
impl TryFrom<&Value> for time::OffsetDateTime {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::PosixTimestamp { seconds, micros } => Ok(Self::from_unix_timestamp(*seconds)
                .map_err(|error| Error::Protocol(format!("invalid POSIX timestamp: {error}")))?
                .replace_nanosecond(micros * 1_000)
                .expect("microseconds are bounded by the protocol")),
            Value::Timestamp(value) => Ok(parse_time_primitive(value)?.assume_utc()),
            other => Err(Error::Protocol(format!(
                "cannot convert {other:?} to time::OffsetDateTime"
            ))),
        }
    }
}

#[cfg(feature = "time")]
fn parse_time_primitive(value: &str) -> Result<time::PrimitiveDateTime> {
    let normalized = format!("{}Z", value.replace(' ', "T"));
    time::PrimitiveDateTime::parse(&normalized, &time::format_description::well_known::Rfc3339)
        .map_err(|error| Error::Protocol(format!("invalid timestamp {value:?}: {error}")))
}

#[cfg(all(
    test,
    any(
        feature = "bigdecimal",
        feature = "chrono",
        feature = "rust_decimal",
        feature = "time",
        feature = "uuid"
    )
))]
mod tests {
    use super::*;

    #[cfg(feature = "uuid")]
    #[test]
    fn uuid_round_trip() {
        let uuid = uuid::Uuid::new_v4();
        assert_eq!(uuid::Uuid::try_from(&Value::from(uuid)).unwrap(), uuid);
    }

    #[cfg(feature = "rust_decimal")]
    #[test]
    fn rust_decimal_round_trip() {
        let decimal = "1234567890.123456789"
            .parse::<rust_decimal::Decimal>()
            .unwrap();
        assert_eq!(
            rust_decimal::Decimal::try_from(&Value::from(decimal)).unwrap(),
            decimal
        );
    }

    #[cfg(feature = "bigdecimal")]
    #[test]
    fn bigdecimal_round_trip() {
        let decimal = "1234567890.123456789"
            .parse::<bigdecimal::BigDecimal>()
            .unwrap();
        assert_eq!(
            bigdecimal::BigDecimal::try_from(&Value::from(decimal.clone())).unwrap(),
            decimal
        );
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn chrono_round_trip() {
        let timestamp = chrono::DateTime::from_timestamp(1_794_230_055, 123_456_000).unwrap();
        assert_eq!(
            chrono::DateTime::<chrono::Utc>::try_from(&Value::from(timestamp)).unwrap(),
            timestamp
        );
        assert_eq!(
            Value::from(timestamp.naive_utc()),
            Value::Timestamp("2026-11-09 13:14:15.123456".into())
        );
    }

    #[cfg(feature = "time")]
    #[test]
    fn time_round_trip() {
        let timestamp = time::OffsetDateTime::from_unix_timestamp(1_794_230_055)
            .unwrap()
            .replace_nanosecond(123_456_000)
            .unwrap();
        assert_eq!(
            time::OffsetDateTime::try_from(&Value::from(timestamp)).unwrap(),
            timestamp
        );
        assert_eq!(
            Value::from(time::PrimitiveDateTime::new(
                timestamp.date(),
                timestamp.time()
            )),
            Value::Timestamp("2026-11-09 13:14:15.123456".into())
        );
    }
}
