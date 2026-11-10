use crate::{Error, Result, Value};

const STRING: u8 = 0x01;
const UNICODE: u8 = 0x02;
const POSITIVE_INTEGER: u8 = 0x04;
const NEGATIVE_INTEGER: u8 = 0x05;
const POSITIVE_DECIMAL: u8 = 0x06;
const NEGATIVE_DECIMAL: u8 = 0x07;
const COMPACT_FLOAT: u8 = 0x08;
const IEEE_DOUBLE: u8 = 0x09;
const OREF: u8 = 0x19;
const BY_REFERENCE: u8 = 0x20;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ListItem<'a> {
    tag: u8,
    data: &'a [u8],
    is_null: bool,
    by_reference: bool,
}

impl<'a> ListItem<'a> {
    pub(crate) fn parse(input: &mut &'a [u8]) -> Result<Self> {
        let original = *input;
        let length = take_u8(input)?;
        let (tag, data, is_null, by_reference) = match length {
            0 => {
                let encoded_length = take_u16(input)? as usize;
                let data_length = encoded_length.checked_sub(1).ok_or_else(|| {
                    Error::Protocol("extended list item has a zero encoded length".into())
                })?;
                let raw_tag = take_u8(input)?;
                let data = take(input, data_length)?;
                (
                    raw_tag & !BY_REFERENCE,
                    data,
                    false,
                    raw_tag & BY_REFERENCE != 0,
                )
            }
            1 => (0, &[][..], true, false),
            short_length => {
                let data_length = (short_length as usize).checked_sub(2).ok_or_else(|| {
                    Error::Protocol(format!("invalid short list item length {short_length}"))
                })?;
                let raw_tag = take_u8(input)?;
                let data = take(input, data_length)?;
                (
                    raw_tag & !BY_REFERENCE,
                    data,
                    false,
                    raw_tag & BY_REFERENCE != 0,
                )
            }
        };
        let consumed = original.len() - input.len();
        let _ = consumed;
        Ok(Self {
            tag,
            data,
            is_null,
            by_reference,
        })
    }

    pub(crate) fn is_null(self) -> bool {
        self.is_null
    }

    pub(crate) fn is_empty(self) -> bool {
        self.tag == STRING && self.data.is_empty()
    }

    pub(crate) fn data(self) -> &'a [u8] {
        self.data
    }

    pub(crate) fn by_reference(self) -> bool {
        self.by_reference
    }

    pub(crate) fn as_i64(self) -> Result<i64> {
        if self.is_null() || self.is_empty() {
            return Ok(0);
        }
        match self.tag {
            POSITIVE_INTEGER => Ok(unsigned_magnitude(self.data, false)? as i64),
            NEGATIVE_INTEGER => {
                let magnitude = unsigned_magnitude(self.data, true)?;
                if magnitude == (i64::MAX as u64) + 1 {
                    Ok(i64::MIN)
                } else {
                    Ok(-(magnitude as i64))
                }
            }
            STRING | UNICODE | OREF | POSITIVE_DECIMAL | NEGATIVE_DECIMAL => {
                let value = self.as_decimal_string()?;
                value.parse().map_err(|_| {
                    Error::Protocol(format!("list item {value:?} is not a signed integer"))
                })
            }
            COMPACT_FLOAT | IEEE_DOUBLE => Ok(self.as_f64()? as i64),
            other => Err(Error::Protocol(format!(
                "cannot decode list item tag {other:#x} as an integer"
            ))),
        }
    }

    pub(crate) fn as_u32(self) -> Result<u32> {
        let value = self.as_i64()?;
        u32::try_from(value)
            .map_err(|_| Error::Protocol(format!("integer {value} does not fit in u32")))
    }

    pub(crate) fn as_string(self) -> Result<String> {
        match self.tag {
            STRING | OREF => Ok(match std::str::from_utf8(self.data) {
                Ok(value) => value.into(),
                Err(_) => self.data.iter().map(|byte| char::from(*byte)).collect(),
            }),
            UNICODE => {
                if !self.data.len().is_multiple_of(2) {
                    return Err(Error::Protocol(
                        "Unicode list item has an odd byte length".into(),
                    ));
                }
                let units = self
                    .data
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_le_bytes(*pair));
                char::decode_utf16(units)
                    .map(|item| {
                        item.map_err(|_| {
                            Error::Protocol("Unicode list item contains invalid UTF-16".into())
                        })
                    })
                    .collect()
            }
            POSITIVE_DECIMAL | NEGATIVE_DECIMAL => self.as_decimal_string(),
            POSITIVE_INTEGER | NEGATIVE_INTEGER => Ok(self.as_i64()?.to_string()),
            COMPACT_FLOAT | IEEE_DOUBLE => Ok(self.as_f64()?.to_string()),
            other => Err(Error::Protocol(format!(
                "cannot decode list item tag {other:#x} as a string"
            ))),
        }
    }

    pub(crate) fn as_decimal_string(self) -> Result<String> {
        match self.tag {
            STRING | UNICODE | OREF => self.as_string(),
            POSITIVE_INTEGER | NEGATIVE_INTEGER => Ok(self.as_i64()?.to_string()),
            POSITIVE_DECIMAL | NEGATIVE_DECIMAL => {
                if self.data.is_empty() {
                    return Err(Error::Protocol(
                        "decimal list item has no scale byte".into(),
                    ));
                }
                let exponent = i32::from(self.data[0] as i8);
                let negative = self.tag == NEGATIVE_DECIMAL;
                let coefficient = unsigned_magnitude(&self.data[1..], negative)?.to_string();
                let mut value = apply_decimal_exponent(&coefficient, exponent);
                if negative && coefficient != "0" {
                    value.insert(0, '-');
                }
                Ok(value)
            }
            COMPACT_FLOAT | IEEE_DOUBLE => Ok(self.as_f64()?.to_string()),
            other => Err(Error::Protocol(format!(
                "cannot decode list item tag {other:#x} as a decimal"
            ))),
        }
    }

    pub(crate) fn as_f64(self) -> Result<f64> {
        match self.tag {
            COMPACT_FLOAT => decode_compact_float(self.data),
            IEEE_DOUBLE => decode_ieee_double(self.data),
            POSITIVE_DECIMAL | NEGATIVE_DECIMAL | STRING | UNICODE => {
                let value = self.as_decimal_string()?;
                value.parse().map_err(|_| {
                    Error::Protocol(format!(
                        "list item {value:?} is not a floating-point number"
                    ))
                })
            }
            POSITIVE_INTEGER | NEGATIVE_INTEGER => Ok(self.as_i64()? as f64),
            other => Err(Error::Protocol(format!(
                "cannot decode list item tag {other:#x} as a float"
            ))),
        }
    }
}

pub(crate) const MAX_LIST_DATA_LEN: usize = 65_534;

pub(crate) fn encoded_text_len(value: &str, empty_as_nul: bool) -> usize {
    if empty_as_nul && value.is_empty() {
        1
    } else if value.chars().any(|character| character as u32 > 255) {
        value.encode_utf16().count() * 2
    } else {
        value.len()
    }
}

pub(crate) fn validate_parameter(value: &Value) -> Result<()> {
    let encoded_length = match value {
        Value::Decimal(value) | Value::String(value) => encoded_text_len(value, true),
        Value::Date(value) | Value::Time(value) | Value::Timestamp(value) => {
            encoded_text_len(value, true)
        }
        Value::Bytes(value) => value.len(),
        _ => 0,
    };
    if encoded_length > MAX_LIST_DATA_LEN {
        return Err(Error::Unsupported(
            "parameters over 65,534 encoded bytes require stream parameters",
        ));
    }
    Ok(())
}

pub(crate) fn encode_string(output: &mut Vec<u8>, value: &str) {
    encode_text(output, value, false);
}

pub(crate) fn encode_raw_string(output: &mut Vec<u8>, value: &[u8]) {
    encode_bytes(output, STRING, value, false);
}

pub(crate) fn encode_item(output: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Null => output.push(1),
        Value::Bool(value) => encode_unsigned(output, u64::from(*value)),
        Value::I16(value) => encode_signed(output, i64::from(*value)),
        Value::I32(value) => encode_signed(output, i64::from(*value)),
        Value::I64(value) => encode_signed(output, *value),
        Value::F32(value) => encode_bytes(output, COMPACT_FLOAT, &value.to_le_bytes(), false),
        Value::F64(value) => encode_bytes(output, IEEE_DOUBLE, &value.to_le_bytes(), false),
        Value::Decimal(value) => encode_text(output, value, true),
        Value::String(value) => encode_text(output, value, true),
        Value::Bytes(value) => encode_raw_string(output, value),
        Value::Date(value) | Value::Time(value) | Value::Timestamp(value) => {
            encode_text(output, value, true);
        }
        Value::PosixTimestamp { seconds, micros } => {
            let micros = i128::from(*seconds) * 1_000_000 + i128::from(*micros);
            if let Ok(value) = i64::try_from(micros) {
                let encoded = if value >= 0 {
                    value ^ 0x1000_0000_0000_0000
                } else {
                    value & !0x6000_0000_0000_0000
                };
                encode_signed(output, encoded);
            } else {
                encode_text(output, &micros.to_string(), true);
            }
        }
    }
}

pub(crate) fn encode_signed(output: &mut Vec<u8>, value: i64) {
    if value >= 0 {
        encode_unsigned(output, value as u64);
        return;
    }
    let mut magnitude = value.unsigned_abs() - 1;
    let mut data = Vec::new();
    loop {
        data.push(!(magnitude as u8));
        magnitude >>= 8;
        if magnitude == 0 {
            break;
        }
    }
    encode_bytes(output, NEGATIVE_INTEGER, &data, false);
}

fn encode_unsigned(output: &mut Vec<u8>, mut value: u64) {
    let mut data = Vec::new();
    while value > 0 {
        data.push(value as u8);
        value >>= 8;
    }
    encode_bytes(output, POSITIVE_INTEGER, &data, false);
}

fn encode_text(output: &mut Vec<u8>, value: &str, empty_as_nul: bool) {
    if empty_as_nul && value.is_empty() {
        encode_bytes(output, STRING, b"\0", false);
    } else if value.chars().any(|character| character as u32 > 255) {
        let mut data = Vec::with_capacity(value.len() * 2);
        for unit in value.encode_utf16() {
            data.extend_from_slice(&unit.to_le_bytes());
        }
        encode_bytes(output, UNICODE, &data, false);
    } else {
        encode_bytes(output, STRING, value.as_bytes(), false);
    }
}

fn encode_bytes(output: &mut Vec<u8>, tag: u8, data: &[u8], by_reference: bool) {
    let tag = if by_reference {
        tag | BY_REFERENCE
    } else {
        tag
    };
    if data.len() > 253 {
        let encoded_length = data.len() + 1;
        let encoded_length = u16::try_from(encoded_length)
            .expect("IRIS list items are limited to 65,534 data bytes");
        output.push(0);
        output.extend_from_slice(&encoded_length.to_le_bytes());
    } else {
        output.push((data.len() + 2) as u8);
    }
    output.push(tag);
    output.extend_from_slice(data);
}

fn take<'a>(input: &mut &'a [u8], length: usize) -> Result<&'a [u8]> {
    if input.len() < length {
        return Err(Error::Protocol(format!(
            "truncated list item: need {length} bytes, have {}",
            input.len()
        )));
    }
    let (value, remaining) = input.split_at(length);
    *input = remaining;
    Ok(value)
}

fn take_u8(input: &mut &[u8]) -> Result<u8> {
    Ok(take(input, 1)?[0])
}

fn take_u16(input: &mut &[u8]) -> Result<u16> {
    let bytes = take(input, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn unsigned_magnitude(data: &[u8], negative: bool) -> Result<u64> {
    if data.len() > 8 {
        return Err(Error::Protocol(format!(
            "integer list item has {} bytes, exceeding the supported 8",
            data.len()
        )));
    }
    if negative {
        let mut magnitude = 0_u64;
        for (shift, byte) in data.iter().enumerate() {
            magnitude |= u64::from(!byte) << (shift * 8);
        }
        magnitude
            .checked_add(1)
            .ok_or_else(|| Error::Protocol("negative integer magnitude exceeds u64::MAX".into()))
    } else {
        Ok(data
            .iter()
            .enumerate()
            .map(|(shift, byte)| u64::from(*byte) << (shift * 8))
            .sum())
    }
}

fn apply_decimal_exponent(coefficient: &str, exponent: i32) -> String {
    if exponent >= 0 {
        let mut value = String::from(coefficient);
        value.extend(std::iter::repeat_n('0', exponent as usize));
        return value;
    }
    let decimal_position = coefficient.len() as i64 + i64::from(exponent);
    if decimal_position <= 0 {
        format!(
            "0.{}{}",
            "0".repeat((-decimal_position) as usize),
            coefficient
        )
    } else {
        let decimal_position = decimal_position as usize;
        format!(
            "{}.{}",
            &coefficient[..decimal_position],
            &coefficient[decimal_position..]
        )
    }
}

fn decode_compact_float(data: &[u8]) -> Result<f64> {
    if data.is_empty() {
        return Ok(0.0);
    }
    if data == [0x80, 0x7f] {
        return Ok(f64::INFINITY);
    }
    if data == [0x80, 0xff] {
        return Ok(f64::NEG_INFINITY);
    }
    match data.len() {
        1..=4 => {
            let mut bytes = [0_u8; 4];
            bytes[4 - data.len()..].copy_from_slice(data);
            Ok(f32::from_le_bytes(bytes) as f64)
        }
        8 => Ok(f64::from_le_bytes(data.try_into().unwrap())),
        length => Err(Error::Protocol(format!(
            "invalid compact float payload length {length}"
        ))),
    }
}

fn decode_ieee_double(data: &[u8]) -> Result<f64> {
    if data.is_empty() {
        return Ok(0.0);
    }
    if data == [0xf8, 0x7f] {
        return Ok(f64::NAN);
    }
    if data == [0xf8, 0xff] {
        return Ok(-f64::NAN);
    }
    if data.len() != 8 {
        return Err(Error::Protocol(format!(
            "invalid IEEE double payload length {}",
            data.len()
        )));
    }
    Ok(f64::from_le_bytes(data.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(value: Value, expected_tag: u8) -> ListItem<'static> {
        let mut encoded = Vec::new();
        encode_item(&mut encoded, &value);
        let leaked: &'static [u8] = Box::leak(encoded.into_boxed_slice());
        let mut input = leaked;
        let item = ListItem::parse(&mut input).unwrap();
        assert!(input.is_empty());
        assert_eq!(item.tag, expected_tag);
        item
    }

    #[test]
    fn scalar_values_round_trip() {
        assert_eq!(
            round_trip(Value::I64(0), POSITIVE_INTEGER)
                .as_i64()
                .unwrap(),
            0
        );
        assert_eq!(
            round_trip(Value::I64(16_456_456), POSITIVE_INTEGER)
                .as_i64()
                .unwrap(),
            16_456_456
        );
        assert_eq!(
            round_trip(Value::I64(-1), NEGATIVE_INTEGER)
                .as_i64()
                .unwrap(),
            -1
        );
        assert_eq!(
            round_trip(Value::I64(-256), NEGATIVE_INTEGER)
                .as_i64()
                .unwrap(),
            -256
        );
        assert_eq!(
            round_trip(Value::I64(i64::MIN), NEGATIVE_INTEGER)
                .as_i64()
                .unwrap(),
            i64::MIN
        );
        assert_eq!(
            round_trip(Value::Bool(true), POSITIVE_INTEGER)
                .as_i64()
                .unwrap(),
            1
        );
        assert_eq!(
            round_trip(Value::from(" café 東京"), UNICODE)
                .as_string()
                .unwrap(),
            " café 東京"
        );
        assert_eq!(
            round_trip(Value::F64(42.5), IEEE_DOUBLE).as_f64().unwrap(),
            42.5
        );
        assert!(round_trip(Value::Null, 0).is_null());
    }

    #[test]
    fn compact_float_padding_and_empty_metadata_match_server_behavior() {
        assert_eq!(decode_compact_float(&[0x2a, 0x42]).unwrap(), 42.5);
        let mut encoded = Vec::new();
        encode_string(&mut encoded, "");
        let mut input = encoded.as_slice();
        assert_eq!(ListItem::parse(&mut input).unwrap().as_i64().unwrap(), 0);
    }

    #[test]
    fn oversized_parameters_are_rejected_without_panicking() {
        let error = validate_parameter(&Value::Bytes(vec![0; MAX_LIST_DATA_LEN + 1])).unwrap_err();
        assert!(matches!(error, Error::Unsupported(_)));
    }

    #[test]
    fn long_and_by_reference_items_decode() {
        let data = vec![b'x'; 300];
        let mut encoded = Vec::new();
        encode_bytes(&mut encoded, STRING, &data, true);
        let mut input = encoded.as_slice();
        let item = ListItem::parse(&mut input).unwrap();
        assert_eq!(item.data(), data);
        assert!(item.by_reference());
        assert!(input.is_empty());
    }

    #[test]
    fn decimal_exponents_do_not_use_floats() {
        let mut encoded = Vec::new();
        encode_bytes(&mut encoded, POSITIVE_DECIMAL, &[3, 0x85, 0x1c], false);
        let mut input = encoded.as_slice();
        let item = ListItem::parse(&mut input).unwrap();
        assert_eq!(item.as_decimal_string().unwrap(), "7301000");

        let mut encoded = Vec::new();
        encode_bytes(&mut encoded, NEGATIVE_DECIMAL, &[0xfc, 0xfe], false);
        let mut input = encoded.as_slice();
        let item = ListItem::parse(&mut input).unwrap();
        assert_eq!(item.as_decimal_string().unwrap(), "-0.0002");
    }

    #[test]
    fn truncated_items_are_errors() {
        let bytes = [10, STRING, b'a'];
        let mut input = bytes.as_slice();
        assert!(ListItem::parse(&mut input).is_err());
    }
}
