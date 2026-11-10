use crate::{Error, Result};

pub(crate) fn placeholder_count(sql: &str) -> Result<usize> {
    match rewrite_placeholders(sql, 0) {
        Ok(_) => Ok(0),
        Err(Error::ParameterCount { expected, .. }) => Ok(expected),
        Err(error) => Err(error),
    }
}

pub(crate) fn rewrite_placeholders(sql: &str, parameter_count: usize) -> Result<String> {
    let bytes = sql.as_bytes();
    let mut output = Vec::with_capacity(sql.len() + parameter_count * 12);
    let mut index = 0;
    let mut found = 0;
    let mut state = State::Code;
    while index < bytes.len() {
        let byte = bytes[index];
        match state {
            State::Code => match byte {
                b'\'' => {
                    state = State::SingleQuote;
                    output.push(b'\'');
                }
                b'"' => {
                    state = State::DoubleQuote;
                    output.push(b'"');
                }
                b'`' => {
                    state = State::Backtick;
                    output.push(b'`');
                }
                b'-' if bytes.get(index + 1) == Some(&b'-') => {
                    state = State::LineComment;
                    output.extend_from_slice(b"--");
                    index += 1;
                }
                b'/' if bytes.get(index + 1) == Some(&b'*') => {
                    state = State::BlockComment;
                    output.extend_from_slice(b"/*");
                    index += 1;
                }
                b'?' => {
                    found += 1;
                    output.extend_from_slice(format!(" :%qpar({found}) ").as_bytes());
                }
                _ => output.push(byte),
            },
            State::SingleQuote | State::DoubleQuote | State::Backtick => {
                output.push(byte);
                let delimiter = match state {
                    State::SingleQuote => b'\'',
                    State::DoubleQuote => b'"',
                    State::Backtick => b'`',
                    _ => unreachable!(),
                };
                if byte == delimiter {
                    if bytes.get(index + 1) == Some(&delimiter) {
                        output.push(delimiter);
                        index += 1;
                    } else {
                        state = State::Code;
                    }
                }
            }
            State::LineComment => {
                output.push(byte);
                if byte == b'\n' {
                    state = State::Code;
                }
            }
            State::BlockComment => {
                output.push(byte);
                if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    output.push(b'/');
                    index += 1;
                    state = State::Code;
                }
            }
        }
        index += 1;
    }
    if state != State::Code && state != State::LineComment {
        return Err(Error::Protocol(
            "unterminated SQL quote or block comment".into(),
        ));
    }
    if found != parameter_count {
        return Err(Error::ParameterCount {
            expected: found,
            actual: parameter_count,
        });
    }
    String::from_utf8(output)
        .map_err(|_| Error::Protocol("placeholder rewriting corrupted SQL UTF-8".into()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Code,
    SingleQuote,
    DoubleQuote,
    Backtick,
    LineComment,
    BlockComment,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_only_real_placeholders() {
        let sql = "SELECT '?', \"?\", value -- ?\nFROM t /* ? */ WHERE id = ? AND note = 'it''s ?'";
        let rewritten = rewrite_placeholders(sql, 1).unwrap();
        assert_eq!(
            rewritten,
            "SELECT '?', \"?\", value -- ?\nFROM t /* ? */ WHERE id =  :%qpar(1)  AND note = 'it''s ?'"
        );
    }

    #[test]
    fn rejects_parameter_count_mismatch() {
        let error = rewrite_placeholders("SELECT ?, ?", 1).unwrap_err();
        assert!(matches!(
            error,
            Error::ParameterCount {
                expected: 2,
                actual: 1
            }
        ));
    }

    #[test]
    fn preserves_unicode() {
        let rewritten = rewrite_placeholders("SELECT N' café ' WHERE id = ?", 1).unwrap();
        assert_eq!(rewritten, "SELECT N' café ' WHERE id =  :%qpar(1) ");
    }
}
