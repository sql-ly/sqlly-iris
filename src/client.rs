use std::{sync::Arc, time::Duration};

use tokio::{net::TcpStream, time::timeout};

use crate::{
    Column, Config, Encryption, Error, Nullable, PROTOCOL_VERSION, Result, Row, SqlType, Value,
    protocol::{
        Frame, ListItem, Operation, encode_item, encode_raw_string, encode_string, read_frame,
        rewrite_placeholders, write_frame,
    },
};

const OPTION_FAST_SELECT: i64 = 1;
const OPTION_DURABLE_TRANSACTIONS: i64 = 4;
const OPTION_REDIRECT_OUTPUT: i64 = 32;
const SQL_TEXT_CHUNK_SIZE: usize = 31_904;

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub protocol_version: u16,
    pub unicode: bool,
    pub locale: String,
    pub product_info: String,
    pub delimited_identifiers: bool,
    pub isolation_level: i64,
    pub job_number: String,
    pub sql_empty_string: i64,
    pub feature_options: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecuteResult {
    pub rows_affected: u64,
}

pub struct Client {
    stream: TcpStream,
    message_count: u32,
    statement_count: u32,
    query_timeout: Duration,
    max_rows: u32,
    server_info: ServerInfo,
    stream_active: bool,
    unusable: Option<&'static str>,
}

impl Client {
    pub async fn connect(config: Config) -> Result<Self> {
        config.validate()?;
        if config.encryption != Encryption::Disabled {
            return Err(Error::Unsupported(
                "TLS transport is not implemented; refusing to downgrade to plaintext",
            ));
        }
        let address = (config.host.as_str(), config.port);
        let stream = if config.connect_timeout.is_zero() {
            TcpStream::connect(address).await?
        } else {
            timeout(config.connect_timeout, TcpStream::connect(address))
                .await
                .map_err(|_| {
                    Error::Io(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "IRIS TCP connection timed out",
                    ))
                })??
        };
        stream.set_nodelay(true)?;
        let mut client = Self {
            stream,
            message_count: 0,
            statement_count: 1,
            query_timeout: config.query_timeout,
            max_rows: config.max_rows,
            server_info: ServerInfo {
                protocol_version: 0,
                unicode: false,
                locale: String::new(),
                product_info: String::new(),
                delimited_identifiers: false,
                isolation_level: 0,
                job_number: String::new(),
                sql_empty_string: 0,
                feature_options: 0,
            },
            stream_active: false,
            unusable: None,
        };
        client.handshake().await?;
        client.open_session(&config).await?;
        Ok(client)
    }

    pub fn server_info(&self) -> &ServerInfo {
        &self.server_info
    }

    pub async fn query(&mut self, sql: &str, parameters: &[Value]) -> Result<QueryStream<'_>> {
        self.ensure_usable()?;
        for parameter in parameters {
            crate::protocol::validate_parameter(parameter)?;
        }
        let sql = rewrite_placeholders(sql, parameters.len())?;
        let statement_id = self.next_statement_id();
        let mut payload = Vec::new();
        encode_sql_text(&mut payload, &sql);
        encode_query_parameters(&mut payload, parameters);
        encode_i64(
            &mut payload,
            self.query_timeout.as_secs().try_into().unwrap_or(i64::MAX),
        );
        encode_i64(&mut payload, i64::from(self.max_rows));
        let metadata = self
            .exchange(Operation::DirectQuery, statement_id, &payload)
            .await?;
        self.check_status(&metadata).await?;
        let response_count = metadata.count;
        let mut input = metadata.payload.as_slice();
        let feature = StatementFeature::parse(&mut input)?;
        let columns = Arc::new(parse_columns(&mut input, &feature)?);
        let _parameter_count = read_i64(&mut input)?;
        let _parameter_flag = read_i64(&mut input)?;
        let data = self.read_response(response_count).await?;
        self.check_status(&data).await?;
        self.stream_active = true;
        Ok(QueryStream {
            client: self,
            statement_id,
            columns,
            feature,
            buffer: data.payload,
            offset: 0,
            status: data.status as i16,
            finished: false,
        })
    }

    pub async fn execute(&mut self, sql: &str, parameters: &[Value]) -> Result<ExecuteResult> {
        self.ensure_usable()?;
        for parameter in parameters {
            crate::protocol::validate_parameter(parameter)?;
        }
        let sql = rewrite_placeholders(sql, parameters.len())?;
        let statement_id = self.next_statement_id();
        let mut payload = Vec::new();
        encode_sql_text(&mut payload, &sql);
        encode_i64(&mut payload, parameters.len() as i64);
        for _ in parameters {
            encode_i64(&mut payload, 99);
            encode_i64(&mut payload, 1);
        }
        let response = self
            .exchange(Operation::Prepare, statement_id, &payload)
            .await?;
        self.check_status(&response).await?;
        let mut input = response.payload.as_slice();
        let feature = StatementFeature::parse(&mut input)?;
        if feature.option & 2 != 0 {
            return Err(Error::Unsupported(
                "fast-insert response decoding is not implemented",
            ));
        }
        let parameter_count = read_i64(&mut input)?;
        if parameter_count < 0 {
            return Err(Error::Protocol("negative update parameter count".into()));
        }
        for _ in 0..parameter_count {
            for _ in 0..4 {
                let _ = ListItem::parse(&mut input)?;
            }
        }
        let _flags = read_i64(&mut input)?;

        let mut execution = Vec::new();
        encode_string(&mut execution, "");
        encode_i64(&mut execution, 0);
        encode_i64(&mut execution, 1);
        encode_i64(&mut execution, parameters.len() as i64);
        for parameter in parameters {
            encode_item(&mut execution, parameter);
        }
        let response = self
            .exchange(Operation::PreparedUpdate, statement_id, &execution)
            .await?;
        self.check_status(&response).await?;
        let mut input = response.payload.as_slice();
        let rows_affected = read_i64(&mut input)?;
        Ok(ExecuteResult {
            rows_affected: u64::try_from(rows_affected).unwrap_or(0),
        })
    }

    pub async fn begin(&mut self) -> Result<Transaction<'_>> {
        self.execute("START TRANSACTION", &[]).await?;
        Ok(Transaction {
            client: self,
            finished: false,
        })
    }

    pub async fn close(mut self) -> Result<()> {
        let count = self.next_count();
        write_frame(&mut self.stream, Operation::Disconnect, count, 0, &[]).await?;
        tokio::io::AsyncWriteExt::shutdown(&mut self.stream).await?;
        Ok(())
    }

    async fn handshake(&mut self) -> Result<()> {
        let payload = PROTOCOL_VERSION.to_le_bytes();
        let response = self.exchange(Operation::Handshake, 0, &payload).await?;
        let mut input = response.payload.as_slice();
        self.server_info.protocol_version = take_u16(&mut input)?;
        self.server_info.unicode = take_u16(&mut input)? == 1;
        self.server_info.locale = read_string(&mut input)?;
        Ok(())
    }

    async fn open_session(&mut self, config: &Config) -> Result<()> {
        let mut payload = Vec::new();
        encode_string(&mut payload, &config.namespace);
        encode_raw_string(&mut payload, &encode_credential(&config.username));
        encode_raw_string(&mut payload, &encode_credential(&config.password));
        let machine_user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "rust".into());
        crate::protocol::validate_parameter(&Value::String(machine_user.clone()))?;
        encode_string(&mut payload, &machine_user);
        encode_string(&mut payload, &config.machine_name);
        encode_string(&mut payload, &config.application_name);
        encode_string(&mut payload, "");
        encode_string(&mut payload, "go");
        encode_string(&mut payload, "");
        encode_i64(&mut payload, 1);
        encode_i64(&mut payload, 0);
        encode_i64(
            &mut payload,
            OPTION_FAST_SELECT | OPTION_DURABLE_TRANSACTIONS | OPTION_REDIRECT_OUTPUT,
        );
        let response = self.exchange(Operation::Connect, 0, &payload).await?;
        if response.status == 417 {
            let mut input = response.payload.as_slice();
            let message = read_string(&mut input)?;
            return Err(Error::Server { code: 417, message });
        }
        self.check_status(&response).await?;
        let mut input = response.payload.as_slice();
        self.server_info.product_info = read_string(&mut input)?;
        self.server_info.delimited_identifiers = read_i64(&mut input)? != 0;
        let _ = read_i64(&mut input)?;
        self.server_info.isolation_level = read_i64(&mut input)?;
        self.server_info.job_number = read_string(&mut input)?;
        self.server_info.sql_empty_string = read_i64(&mut input)?;
        self.server_info.feature_options = read_i64(&mut input)?;
        Ok(())
    }

    async fn check_status(&mut self, frame: &Frame) -> Result<()> {
        let code = frame.status as i16;
        if code == 0 || code == 100 {
            return Ok(());
        }
        let message = self.get_server_error(code).await?;
        Err(Error::Server { code, message })
    }

    async fn get_server_error(&mut self, code: i16) -> Result<String> {
        let mut payload = Vec::new();
        encode_i64(&mut payload, i64::from(code));
        let response = self
            .exchange(Operation::GetServerError, 0, &payload)
            .await?;
        let mut input = response.payload.as_slice();
        read_string(&mut input)
    }

    async fn fetch_stream(&mut self, handle: String, binary: bool) -> Result<Value> {
        let statement_id = self.next_statement_id();
        let mut payload = Vec::new();
        encode_string(&mut payload, &handle);
        encode_i64(&mut payload, -1);
        let response = self
            .exchange(Operation::ReadStream, statement_id, &payload)
            .await?;
        self.check_status(&response).await?;
        if binary {
            Ok(Value::Bytes(response.payload))
        } else {
            Ok(Value::String(String::from_utf8(response.payload).map_err(
                |_| Error::Protocol("character stream is not valid UTF-8".into()),
            )?))
        }
    }

    async fn exchange(
        &mut self,
        operation: Operation,
        statement_id: u32,
        payload: &[u8],
    ) -> Result<Frame> {
        let count = self.next_count();
        write_frame(&mut self.stream, operation, count, statement_id, payload).await?;
        self.read_response(count).await
    }

    async fn read_response(&mut self, expected_count: u32) -> Result<Frame> {
        let frame = read_frame(&mut self.stream).await?;
        tracing::trace!(
            count = frame.count,
            statement_id = frame.statement_id,
            status = frame.status,
            payload_len = frame.payload.len(),
            "received IRIS frame"
        );
        if frame.count != expected_count {
            return Err(Error::Protocol(format!(
                "response count {} does not match request count {expected_count}",
                frame.count
            )));
        }
        Ok(frame)
    }

    fn next_statement_id(&mut self) -> u32 {
        let value = self.statement_count;
        self.statement_count = self.statement_count.wrapping_add(1);
        value
    }

    fn next_count(&mut self) -> u32 {
        let value = self.message_count;
        self.message_count = self.message_count.wrapping_add(1);
        value
    }

    fn ensure_usable(&self) -> Result<()> {
        if let Some(reason) = self.unusable {
            return Err(Error::ConnectionUnusable(reason));
        }
        if self.stream_active {
            return Err(Error::ConnectionUnusable(
                "another result stream is still active",
            ));
        }
        Ok(())
    }
}

fn encode_credential(value: &str) -> Vec<u8> {
    let input = value.as_bytes();
    let mut output = vec![0; input.len()];
    for (index, byte) in input.iter().enumerate() {
        let remaining = input.len() - index - 1;
        let transformed = (usize::from(*byte ^ 0xa7) + remaining) as u8;
        output[remaining] = transformed.rotate_left(5);
    }
    output
}

fn encode_sql_text(output: &mut Vec<u8>, sql: &str) {
    if sql.is_empty() {
        encode_string(output, "");
        return;
    }
    let chunks: Vec<&[u8]> = sql.as_bytes().chunks(SQL_TEXT_CHUNK_SIZE).collect();
    encode_i64(output, chunks.len() as i64);
    for chunk in chunks {
        encode_raw_string(output, chunk);
    }
}

fn encode_query_parameters(output: &mut Vec<u8>, parameters: &[Value]) {
    encode_i64(output, parameters.len() as i64);
    for _ in parameters {
        encode_i64(output, 99);
        encode_i64(output, 4);
    }
    encode_i64(output, 1);
    encode_i64(output, parameters.len() as i64);
    for parameter in parameters {
        encode_item(output, parameter);
    }
}

fn encode_i64(output: &mut Vec<u8>, value: i64) {
    encode_item(output, &Value::I64(value));
}

fn take_u16(input: &mut &[u8]) -> Result<u16> {
    if input.len() < 2 {
        return Err(Error::Protocol("truncated u16 in server response".into()));
    }
    let value = u16::from_le_bytes([input[0], input[1]]);
    *input = &input[2..];
    Ok(value)
}

fn read_i64(input: &mut &[u8]) -> Result<i64> {
    ListItem::parse(input)?.as_i64()
}

fn read_string(input: &mut &[u8]) -> Result<String> {
    ListItem::parse(input)?.as_string()
}

#[derive(Debug)]
struct StatementFeature {
    option: i64,
    max_row_item_count: usize,
}

impl StatementFeature {
    fn parse(input: &mut &[u8]) -> Result<Self> {
        let option = read_i64(input)?;
        if option == 2 {
            let _message_count = read_i64(input)?;
        }
        let max_row_item_count = if option == 1 || option == 2 {
            let count = read_i64(input)?;
            usize::try_from(count)
                .map_err(|_| Error::Protocol("invalid fast-select row item count".into()))?
        } else {
            0
        };
        Ok(Self {
            option,
            max_row_item_count,
        })
    }
}

fn parse_columns(input: &mut &[u8], feature: &StatementFeature) -> Result<Vec<Column>> {
    let count = read_i64(input)?;
    let count = usize::try_from(count)
        .map_err(|_| Error::Protocol("negative result column count".into()))?;
    let mut columns = Vec::with_capacity(count);
    for index in 0..count {
        let name = read_string(input)?;
        let sql_type = SqlType::from_code(
            read_i64(input)?
                .try_into()
                .map_err(|_| Error::Protocol("SQL type code does not fit in i16".into()))?,
        );
        let precision = ListItem::parse(input)?.as_u32()?;
        let scale = read_i64(input)?
            .try_into()
            .map_err(|_| Error::Protocol("column scale does not fit in i32".into()))?;
        let nullable = match read_i64(input)? {
            0 => Nullable::NoNulls,
            1 => Nullable::Nullable,
            _ => Nullable::Unknown,
        };
        let label = read_string(input)?;
        let table_name = read_string(input)?;
        let schema = read_string(input)?;
        let catalog = read_string(input)?;
        let additional = ListItem::parse(input)?;
        let flags = additional.data();
        if flags.len() < 4 {
            return Err(Error::Protocol(format!(
                "column {name:?} has only {} metadata flags",
                flags.len()
            )));
        }
        let slot_position = if feature.option & OPTION_FAST_SELECT != 0 {
            let position = read_i64(input)?;
            usize::try_from(position - 1)
                .map_err(|_| Error::Protocol("invalid fast-select slot position".into()))?
        } else {
            index
        };
        columns.push(Column {
            name,
            sql_type,
            precision,
            scale,
            nullable,
            label,
            table_name,
            schema,
            catalog,
            is_auto_increment: flags[0] == 1,
            is_case_sensitive: flags[1] == 1,
            is_currency: flags[2] == 1,
            is_read_only: flags[3] == 1,
            is_row_id: flags.get(11) == Some(&1),
            slot_position,
        });
    }
    Ok(columns)
}

enum DecodedCell {
    Value(Value),
    Stream { handle: String, binary: bool },
}

fn decode_row(
    buffer: &[u8],
    offset: &mut usize,
    columns: &[Column],
    feature: &StatementFeature,
) -> Result<Vec<DecodedCell>> {
    let mut input = &buffer[*offset..];
    let items = if feature.option & OPTION_FAST_SELECT != 0 {
        let packed = ListItem::parse(&mut input)?;
        let mut packed_input = packed.data();
        let mut items = Vec::with_capacity(feature.max_row_item_count);
        for _ in 0..feature.max_row_item_count {
            items.push(ListItem::parse(&mut packed_input)?);
        }
        items
    } else {
        let mut items = Vec::with_capacity(columns.len());
        for _ in 0..columns.len() {
            items.push(ListItem::parse(&mut input)?);
        }
        items
    };
    *offset = buffer.len() - input.len();
    columns
        .iter()
        .map(|column| {
            let item = items.get(column.slot_position).copied().ok_or_else(|| {
                Error::Protocol(format!(
                    "column {:?} references missing slot {}",
                    column.name, column.slot_position
                ))
            })?;
            decode_cell(column, item)
        })
        .collect()
}

fn decode_cell(column: &Column, item: ListItem<'_>) -> Result<DecodedCell> {
    if item.by_reference() {
        tracing::trace!(column = %column.name, "decoding by-reference IRIS value");
    }
    if item.is_null() || item.is_empty() {
        return Ok(DecodedCell::Value(Value::Null));
    }
    let value = match column.sql_type {
        SqlType::LongVarChar | SqlType::WLongVarChar => {
            return Ok(DecodedCell::Stream {
                handle: item.as_string()?,
                binary: false,
            });
        }
        SqlType::LongVarBinary => {
            return Ok(DecodedCell::Stream {
                handle: item.as_string()?,
                binary: true,
            });
        }
        SqlType::Bit => Value::Bool(item.as_i64()? != 0),
        SqlType::TinyInt | SqlType::SmallInt => {
            Value::I16(item.as_i64()?.try_into().map_err(|_| {
                Error::Protocol(format!("column {:?} does not fit in i16", column.name))
            })?)
        }
        SqlType::Integer => Value::I32(item.as_i64()?.try_into().map_err(|_| {
            Error::Protocol(format!("column {:?} does not fit in i32", column.name))
        })?),
        SqlType::BigInt => Value::I64(item.as_i64()?),
        SqlType::Real | SqlType::Float => Value::F32(item.as_f64()? as f32),
        SqlType::Double => Value::F64(item.as_f64()?),
        SqlType::Numeric | SqlType::Decimal => Value::Decimal(item.as_decimal_string()?),
        SqlType::Binary | SqlType::VarBinary => Value::Bytes(item.data().to_vec()),
        SqlType::Date | SqlType::DateHorolog => Value::Date(item.as_string()?),
        SqlType::Time | SqlType::TimeHorolog => Value::Time(item.as_string()?),
        SqlType::Timestamp => Value::Timestamp(item.as_string()?),
        SqlType::TimestampPosix => {
            let encoded = item.as_i64()?;
            let micros = if encoded > 0 {
                encoded ^ 0x1000_0000_0000_0000
            } else {
                encoded | 0x6000_0000_0000_0000
            };
            Value::PosixTimestamp {
                seconds: micros.div_euclid(1_000_000),
                micros: micros.rem_euclid(1_000_000) as u32,
            }
        }
        SqlType::Guid
        | SqlType::WVarChar
        | SqlType::WChar
        | SqlType::Char
        | SqlType::VarChar
        | SqlType::Unknown(_) => {
            let mut value = item.as_string()?;
            if matches!(column.sql_type, SqlType::VarChar) && value == "\0" {
                value.clear();
            }
            Value::String(value)
        }
    };
    Ok(DecodedCell::Value(value))
}

pub struct QueryStream<'a> {
    client: &'a mut Client,
    statement_id: u32,
    columns: Arc<Vec<Column>>,
    feature: StatementFeature,
    buffer: Vec<u8>,
    offset: usize,
    status: i16,
    finished: bool,
}

impl QueryStream<'_> {
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    pub async fn next_row(&mut self) -> Result<Option<Row>> {
        if self.finished {
            return Ok(None);
        }
        loop {
            if self.offset < self.buffer.len() {
                break;
            }
            if self.status == 100 {
                self.finish();
                return Ok(None);
            }
            let response = self
                .client
                .exchange(Operation::FetchData, self.statement_id, &[])
                .await?;
            self.status = response.status as i16;
            if let Err(error) = self.client.check_status(&response).await {
                self.finish();
                return Err(error);
            }
            self.buffer = response.payload;
            self.offset = 0;
            if self.buffer.is_empty() && self.status == 100 {
                self.finish();
                return Ok(None);
            }
        }
        let decoded = decode_row(&self.buffer, &mut self.offset, &self.columns, &self.feature)?;
        let mut values = Vec::with_capacity(decoded.len());
        for cell in decoded {
            values.push(match cell {
                DecodedCell::Value(value) => value,
                DecodedCell::Stream { handle, binary } => {
                    self.client.fetch_stream(handle, binary).await?
                }
            });
        }
        Ok(Some(Row {
            columns: Arc::clone(&self.columns),
            values,
        }))
    }

    pub async fn collect(mut self) -> Result<Vec<Row>> {
        let mut rows = Vec::new();
        while let Some(row) = self.next_row().await? {
            rows.push(row);
        }
        Ok(rows)
    }

    pub async fn close(mut self) -> Result<()> {
        while self.next_row().await?.is_some() {}
        Ok(())
    }

    fn finish(&mut self) {
        self.finished = true;
        self.client.stream_active = false;
    }
}

impl Drop for QueryStream<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.client.stream_active = false;
            self.client.unusable =
                Some("a result stream was dropped before it was drained; close the connection");
        }
    }
}

pub struct Transaction<'a> {
    client: &'a mut Client,
    finished: bool,
}

impl<'a> Transaction<'a> {
    pub async fn execute(&mut self, sql: &str, parameters: &[Value]) -> Result<ExecuteResult> {
        self.client.execute(sql, parameters).await
    }

    pub async fn query(&mut self, sql: &str, parameters: &[Value]) -> Result<QueryStream<'_>> {
        self.client.query(sql, parameters).await
    }

    pub async fn commit(mut self) -> Result<()> {
        self.finish(Operation::Commit).await
    }

    pub async fn rollback(mut self) -> Result<()> {
        self.finish(Operation::Rollback).await
    }

    async fn finish(&mut self, operation: Operation) -> Result<()> {
        let response = self.client.exchange(operation, 0, &[]).await?;
        self.client.check_status(&response).await?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.client.unusable =
                Some("a transaction was dropped without commit or rollback; close the connection");
        }
    }
}
