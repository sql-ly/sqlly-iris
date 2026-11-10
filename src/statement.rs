use crate::{Client, ExecuteResult, QueryStream, Result, Value, client::PreparedParts};

pub struct PreparedStatement<'a> {
    client: &'a mut Client,
    parts: PreparedParts,
    closed: bool,
}

impl Client {
    pub async fn prepare(&mut self, sql: &str) -> Result<PreparedStatement<'_>> {
        let parts = self.prepare_statement_parts(sql).await?;
        Ok(PreparedStatement {
            client: self,
            parts,
            closed: false,
        })
    }

    pub async fn last_insert_id(&mut self) -> Result<i64> {
        let rows = self
            .query("SELECT LAST_IDENTITY()", &[])
            .await?
            .collect()
            .await?;
        let value = rows
            .into_iter()
            .next()
            .ok_or_else(|| crate::Error::Protocol("LAST_IDENTITY returned no row".into()))?
            .into_values()
            .remove(0);
        match value {
            Value::I16(value) => Ok(i64::from(value)),
            Value::I32(value) => Ok(i64::from(value)),
            Value::I64(value) => Ok(value),
            Value::Decimal(value) => value
                .parse()
                .map_err(|_| crate::Error::Protocol("LAST_IDENTITY is not an integer".into())),
            other => Err(crate::Error::Protocol(format!(
                "LAST_IDENTITY returned an unexpected value: {other:?}"
            ))),
        }
    }
}

impl PreparedStatement<'_> {
    pub fn parameter_count(&self) -> usize {
        self.parts.parameter_count
    }

    pub async fn query(&mut self, parameters: &[Value]) -> Result<QueryStream<'_>> {
        self.client
            .query_prepared_parts(&self.parts, parameters)
            .await
    }

    pub async fn execute(&mut self, parameters: &[Value]) -> Result<ExecuteResult> {
        self.client
            .execute_prepared_parts(&self.parts, parameters)
            .await
    }

    pub async fn execute_many(&mut self, batches: &[Vec<Value>]) -> Result<ExecuteResult> {
        self.client
            .execute_many_prepared_parts(&self.parts, batches)
            .await
    }

    pub async fn close(mut self) -> Result<()> {
        let result = self
            .client
            .close_statement_now(self.parts.statement_id)
            .await;
        self.closed = true;
        result
    }
}

impl Drop for PreparedStatement<'_> {
    fn drop(&mut self) {
        if !self.closed {
            self.client.defer_statement_close(self.parts.statement_id);
        }
    }
}
