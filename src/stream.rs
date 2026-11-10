use crate::{
    Client, Error, Result, Value,
    protocol::{ListItem, Operation, encode_string},
};

impl Client {
    pub async fn read_stream(&mut self, handle: &str, length: i32, binary: bool) -> Result<Value> {
        self.fetch_stream(handle.into(), length, binary).await
    }

    pub async fn stream_size(&mut self, handle: &str) -> Result<u64> {
        let response = self
            .scalar_stream_operation(Operation::StreamSize, handle)
            .await?;
        unsigned_stream_scalar(response.payload.as_slice())
    }

    pub async fn stream_truncate(&mut self, handle: &str, position: u64) -> Result<()> {
        let mut payload = Vec::new();
        encode_string(&mut payload, handle);
        encode_position(&mut payload, position)?;
        self.session_send_payload(Operation::StreamTruncate, &payload)
            .await
    }

    pub async fn stream_set_bytes(
        &mut self,
        handle: &str,
        position: u64,
        data: &[u8],
    ) -> Result<()> {
        if position == 0 {
            return Err(Error::Protocol("stream positions are one-based".into()));
        }
        let mut payload = Vec::new();
        encode_string(&mut payload, handle);
        encode_position(&mut payload, position - 1)?;
        encode_position(&mut payload, data.len() as u64)?;
        payload.extend_from_slice(data);
        self.session_send_payload(Operation::StreamSetBytes, &payload)
            .await
    }

    pub async fn close_stream(&mut self, handle: &str) -> Result<()> {
        self.session_send_payload(Operation::CloseStream, &handle_payload(handle))
            .await
    }

    async fn scalar_stream_operation(
        &mut self,
        operation: Operation,
        handle: &str,
    ) -> Result<crate::protocol::Frame> {
        self.operation(operation, 0, &handle_payload(handle)).await
    }
}

fn handle_payload(handle: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    encode_string(&mut payload, handle);
    payload
}

fn encode_position(output: &mut Vec<u8>, position: u64) -> Result<()> {
    let position = i64::try_from(position)
        .map_err(|_| Error::Protocol("stream position exceeds i64::MAX".into()))?;
    crate::protocol::encode_item(output, &Value::I64(position));
    Ok(())
}

fn unsigned_stream_scalar(payload: &[u8]) -> Result<u64> {
    let mut input = payload;
    let value = ListItem::parse(&mut input)?.as_i64()?;
    u64::try_from(value).map_err(|_| Error::Protocol(format!("negative stream position {value}")))
}
