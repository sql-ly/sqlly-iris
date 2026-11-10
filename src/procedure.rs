use std::sync::Arc;

use crate::{
    Client, Column, Result, Row, Value,
    protocol::{Frame, Operation, encode_item, encode_string},
};

#[derive(Debug, Clone)]
pub struct ProcedureResult {
    pub return_value: Option<Value>,
    pub columns: Arc<Vec<Column>>,
    pub rows: Vec<Row>,
}

pub struct ProcedureCall<'a> {
    client: &'a mut Client,
    statement_id: u32,
    pending: Option<Frame>,
    scalar_result: Option<ProcedureResult>,
    first: bool,
    finished: bool,
}

impl Client {
    pub async fn call_procedure(
        &mut self,
        name: &str,
        parameters: &[Value],
    ) -> Result<ProcedureCall<'_>> {
        for parameter in parameters {
            crate::protocol::validate_parameter(parameter)?;
        }
        let statement_id = self.allocate_statement_id();
        let mut prepare = Vec::new();
        encode_string(&mut prepare, name);
        encode_count(&mut prepare, 0);
        encode_count(&mut prepare, 0);
        encode_count(&mut prepare, 0);
        encode_count(&mut prepare, 1);
        encode_values(&mut prepare, parameters);
        let prepared = self
            .operation(Operation::DirectStoredProcedure, statement_id, &prepare)
            .await?;
        let mut probe = prepared.payload.as_slice();
        let result_kind = crate::client::read_i64(&mut probe)?;
        if result_kind != -1 {
            return Ok(ProcedureCall {
                client: self,
                statement_id,
                pending: None,
                scalar_result: Some(scalar_result(prepared.payload.as_slice())?),
                first: false,
                finished: false,
            });
        }

        let mut execute = Vec::new();
        encode_count(&mut execute, 0);
        encode_count(&mut execute, 0);
        encode_count(&mut execute, 1);
        encode_values(&mut execute, parameters);
        let first = self
            .operation(
                Operation::MoreStoredProcedureResults,
                statement_id,
                &execute,
            )
            .await?;
        Ok(ProcedureCall {
            client: self,
            statement_id,
            pending: Some(first),
            scalar_result: None,
            first: true,
            finished: false,
        })
    }
}

impl<'a> ProcedureCall<'a> {
    pub async fn next_result(&mut self) -> Result<Option<ProcedureResult>> {
        if self.finished {
            return Ok(None);
        }
        if let Some(result) = self.scalar_result.take() {
            self.finished = true;
            return Ok(Some(result));
        }
        let frame = match self.pending.take() {
            Some(frame) => frame,
            None => {
                let mut payload = Vec::new();
                encode_count(&mut payload, 1);
                self.client
                    .operation(Operation::MoreResults, self.statement_id, &payload)
                    .await?
            }
        };
        let result = self
            .client
            .decode_procedure_result(self.statement_id, frame, self.first)
            .await?;
        if result.is_none() {
            self.finished = true;
        } else {
            self.first = false;
        }
        Ok(result)
    }

    pub async fn collect(mut self) -> Result<Vec<ProcedureResult>> {
        let mut results = Vec::new();
        while let Some(result) = self.next_result().await? {
            results.push(result);
        }
        Ok(results)
    }

    pub fn into_stream(
        self,
    ) -> impl futures_core::Stream<Item = Result<ProcedureResult>> + Send + 'a {
        async_stream::try_stream! {
            let mut call = self;
            while let Some(result) = call.next_result().await? {
                yield result;
            }
        }
    }
}

impl Drop for ProcedureCall<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.client.defer_statement_close(self.statement_id);
        }
    }
}

fn scalar_result(payload: &[u8]) -> Result<ProcedureResult> {
    let mut input = payload;
    while !input.is_empty() {
        let item = crate::protocol::ListItem::parse(&mut input)?;
        if item.as_string()? == "_isc_sp_ret_val" {
            let _sql_type = crate::protocol::ListItem::parse(&mut input)?;
            let return_value = crate::protocol::ListItem::parse(&mut input)?.to_value()?;
            return Ok(ProcedureResult {
                return_value: Some(return_value),
                columns: Arc::new(Vec::new()),
                rows: Vec::new(),
            });
        }
    }
    Err(crate::Error::Protocol(
        "scalar procedure response has no _isc_sp_ret_val value".into(),
    ))
}

fn encode_count(output: &mut Vec<u8>, value: i64) {
    encode_item(output, &Value::I64(value));
}

fn encode_values(output: &mut Vec<u8>, parameters: &[Value]) {
    encode_count(output, parameters.len() as i64);
    for parameter in parameters {
        encode_item(output, parameter);
    }
}
