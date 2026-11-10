use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut show_items = false;
    let mut path = None;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--items" => show_items = true,
            "--help" | "-h" => {
                eprintln!("usage: iris-dissect [--items] CAPTURE.tap");
                return Ok(());
            }
            _ if path.is_none() => path = Some(argument),
            _ => return Err("usage: iris-dissect [--items] CAPTURE.tap".into()),
        }
    }
    let path = path.ok_or("usage: iris-dissect [--items] CAPTURE.tap")?;
    let capture = std::fs::read_to_string(path)?;
    let mut streams = BTreeMap::<(u64, String), Vec<u8>>::new();
    for (line_number, line) in capture.lines().enumerate() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(format!("invalid capture record on line {}", line_number + 1).into());
        }
        let connection = fields[0].parse()?;
        let expected_length: usize = fields[3].parse()?;
        let bytes = decode_hex(fields[4])?;
        if bytes.len() != expected_length {
            return Err(format!("length mismatch on line {}", line_number + 1).into());
        }
        streams
            .entry((connection, fields[1].into()))
            .or_default()
            .extend_from_slice(&bytes);
    }
    for ((connection, direction), bytes) in streams {
        let mut offset = 0;
        let mut frame_index = 0_u64;
        while offset < bytes.len() {
            if bytes.len() - offset < 14 {
                return Err(format!(
                    "connection {connection} {direction}: {} trailing header bytes",
                    bytes.len() - offset
                )
                .into());
            }
            let payload_length =
                u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            let end = offset
                .checked_add(14)
                .and_then(|value| value.checked_add(payload_length))
                .ok_or("frame length overflow")?;
            if end > bytes.len() {
                return Err(format!(
                    "connection {connection} {direction}: truncated frame at offset {offset}"
                )
                .into());
            }
            let count = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
            let statement = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap());
            let marker = [bytes[offset + 12], bytes[offset + 13]];
            let status = u16::from_le_bytes(marker);
            let marker_text: String = marker
                .iter()
                .map(|byte| {
                    if byte.is_ascii_graphic() {
                        char::from(*byte)
                    } else {
                        '.'
                    }
                })
                .collect();
            let payload = &bytes[offset + 14..end];
            let preview_length = payload.len().min(256);
            println!(
                "#{frame_index} connection={connection} direction={direction} count={count} statement={statement} marker={marker_text} status={status} payload_len={payload_length} payload={}{}",
                encode_hex(&payload[..preview_length]),
                if payload.len() > preview_length {
                    "..."
                } else {
                    ""
                }
            );
            if show_items && frame_index != 0 {
                print_items(frame_index, payload);
            } else if show_items && direction == "s>c" && payload.len() >= 4 {
                print_items(frame_index, &payload[4..]);
            }
            offset = end;
            frame_index += 1;
        }
    }
    Ok(())
}

#[derive(Debug)]
struct Item<'a> {
    tag: u8,
    data: &'a [u8],
    null: bool,
    by_reference: bool,
}

fn print_items(frame_index: u64, payload: &[u8]) {
    let mut offset = 0;
    let mut item_index = 0_u64;
    while offset < payload.len() {
        match parse_item(payload, offset) {
            Ok((item, next)) => {
                let text = item_text(&item);
                println!(
                    "  #{frame_index}.{item_index} offset={offset} tag={:#04x} null={} byref={} len={} data={} text={}",
                    item.tag,
                    item.null,
                    item.by_reference,
                    item.data.len(),
                    encode_hex(&item.data[..item.data.len().min(128)]),
                    text
                );
                offset = next;
                item_index += 1;
            }
            Err(error) => {
                println!("  #{frame_index}.{item_index} offset={offset} decode_error={error}");
                break;
            }
        }
    }
}

fn parse_item(data: &[u8], offset: usize) -> Result<(Item<'_>, usize), String> {
    let length = *data
        .get(offset)
        .ok_or_else(|| "missing item length".to_string())?;
    let mut cursor = offset + 1;
    if length == 1 {
        return Ok((
            Item {
                tag: 0,
                data: &[],
                null: true,
                by_reference: false,
            },
            cursor,
        ));
    }
    let data_length = if length == 0 {
        let encoded = data
            .get(cursor..cursor + 2)
            .ok_or_else(|| "truncated extended item length".to_string())?;
        cursor += 2;
        usize::from(u16::from_le_bytes([encoded[0], encoded[1]]))
            .checked_sub(1)
            .ok_or_else(|| "invalid extended item length".to_string())?
    } else {
        usize::from(length)
            .checked_sub(2)
            .ok_or_else(|| "invalid short item length".to_string())?
    };
    let raw_tag = *data
        .get(cursor)
        .ok_or_else(|| "missing item tag".to_string())?;
    cursor += 1;
    let end = cursor
        .checked_add(data_length)
        .ok_or_else(|| "item length overflow".to_string())?;
    let item_data = data
        .get(cursor..end)
        .ok_or_else(|| format!("item needs {data_length} data bytes"))?;
    Ok((
        Item {
            tag: raw_tag & !0x20,
            data: item_data,
            null: false,
            by_reference: raw_tag & 0x20 != 0,
        },
        end,
    ))
}

fn item_text(item: &Item<'_>) -> String {
    if item.null {
        return "NULL".into();
    }
    match item.tag {
        1 | 25 => std::str::from_utf8(item.data)
            .map(|value| format!("{value:?}"))
            .unwrap_or_else(|_| "<binary>".into()),
        2 if item.data.len().is_multiple_of(2) => {
            let units = item
                .data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes(*pair));
            char::decode_utf16(units)
                .collect::<Result<String, _>>()
                .map(|value| format!("{value:?}"))
                .unwrap_or_else(|_| "<invalid UTF-16>".into())
        }
        2 => "<invalid UTF-16>".into(),
        _ => "<untyped>".into(),
    }
}

fn decode_hex(input: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if !input.len().is_multiple_of(2) {
        return Err("odd hexadecimal input length".into());
    }
    input
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let high = nibble(pair[0])?;
            let low = nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn nibble(byte: u8) -> Result<u8, Box<dyn std::error::Error>> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("invalid hexadecimal byte {byte}").into()),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hexadecimal_round_trip() {
        let bytes = [0, 1, 15, 16, 127, 128, 255];
        assert_eq!(decode_hex(&encode_hex(&bytes)).unwrap(), bytes);
    }

    #[test]
    fn list_items_decode_null_short_extended_and_by_reference_forms() {
        let bytes = [1, 3, 1, b'A', 0, 3, 0, 2, 0xe9, 0, 3, 0x21, b'x'];
        let (null, next) = parse_item(&bytes, 0).unwrap();
        assert!(null.null);
        let (short, next) = parse_item(&bytes, next).unwrap();
        assert_eq!(short.data, b"A");
        let (unicode, next) = parse_item(&bytes, next).unwrap();
        assert_eq!(item_text(&unicode), "\"é\"");
        let (by_reference, next) = parse_item(&bytes, next).unwrap();
        assert!(by_reference.by_reference);
        assert_eq!(by_reference.tag, 1);
        assert_eq!(next, bytes.len());
        assert!(parse_item(&[3, 1], 0).is_err());
    }
}
