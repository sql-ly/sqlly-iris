use crate::{Client, Error, Result, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub schema: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub schema: String,
    pub table: String,
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub ordinal_position: u32,
    pub character_maximum_length: Option<u64>,
    pub numeric_precision: Option<u32>,
    pub numeric_scale: Option<u32>,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Procedure {
    pub schema: String,
    pub name: String,
    pub kind: String,
}

impl Client {
    pub async fn list_schemas(&mut self) -> Result<Vec<Schema>> {
        let rows = self
            .query(
                "SELECT SCHEMA_NAME FROM INFORMATION_SCHEMA.SCHEMATA ORDER BY SCHEMA_NAME",
                &[],
            )
            .await?
            .collect()
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok(Schema {
                    name: string_value(row.into_values().remove(0))?,
                })
            })
            .collect()
    }

    pub async fn list_tables(&mut self, schema: &str) -> Result<Vec<Table>> {
        let rows = self
            .query(
                "SELECT TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_SCHEMA = ? ORDER BY TABLE_NAME",
                &[Value::from(schema)],
            )
            .await?
            .collect()
            .await?;
        rows.into_iter()
            .map(|row| {
                let mut values = row.into_values();
                Ok(Table {
                    schema: string_value(values.remove(0))?,
                    name: string_value(values.remove(0))?,
                    kind: string_value(values.remove(0))?,
                })
            })
            .collect()
    }

    pub async fn list_columns(&mut self, schema: &str, table: &str) -> Result<Vec<Column>> {
        let rows = self
            .query(
                "SELECT TABLE_SCHEMA, TABLE_NAME, COLUMN_NAME, DATA_TYPE, IS_NULLABLE, ORDINAL_POSITION, CHARACTER_MAXIMUM_LENGTH, NUMERIC_PRECISION, NUMERIC_SCALE, COLUMN_DEFAULT FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION",
                &[Value::from(schema), Value::from(table)],
            )
            .await?
            .collect()
            .await?;
        rows.into_iter()
            .map(|row| {
                let mut values = row.into_values();
                Ok(Column {
                    schema: string_value(values.remove(0))?,
                    table: string_value(values.remove(0))?,
                    name: string_value(values.remove(0))?,
                    data_type: string_value(values.remove(0))?,
                    nullable: string_value(values.remove(0))? == "YES",
                    ordinal_position: unsigned_value(values.remove(0))?,
                    character_maximum_length: optional_unsigned_value(values.remove(0))?
                        .map(u64::from),
                    numeric_precision: optional_unsigned_value(values.remove(0))?,
                    numeric_scale: optional_unsigned_value(values.remove(0))?,
                    default: optional_string_value(values.remove(0)),
                })
            })
            .collect()
    }

    pub async fn list_procedures(&mut self, schema: &str) -> Result<Vec<Procedure>> {
        let rows = self
            .query(
                "SELECT ROUTINE_SCHEMA, ROUTINE_NAME, ROUTINE_TYPE FROM INFORMATION_SCHEMA.ROUTINES WHERE ROUTINE_SCHEMA = ? ORDER BY ROUTINE_NAME",
                &[Value::from(schema)],
            )
            .await?
            .collect()
            .await?;
        rows.into_iter()
            .map(|row| {
                let mut values = row.into_values();
                Ok(Procedure {
                    schema: string_value(values.remove(0))?,
                    name: string_value(values.remove(0))?,
                    kind: string_value(values.remove(0))?,
                })
            })
            .collect()
    }
}

fn string_value(value: Value) -> Result<String> {
    match value {
        Value::String(value) => Ok(value),
        other => Err(Error::Protocol(format!(
            "expected catalog text, got {other:?}"
        ))),
    }
}

fn optional_string_value(value: Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) => Some(value),
        other => Some(format!("{other:?}")),
    }
}

fn unsigned_value(value: Value) -> Result<u32> {
    match value {
        Value::I16(value) => {
            u32::try_from(value).map_err(|_| Error::Protocol("negative catalog integer".into()))
        }
        Value::I32(value) => {
            u32::try_from(value).map_err(|_| Error::Protocol("negative catalog integer".into()))
        }
        Value::I64(value) => u32::try_from(value)
            .map_err(|_| Error::Protocol("catalog integer is out of range".into())),
        Value::Decimal(value) => value
            .parse()
            .map_err(|_| Error::Protocol("invalid catalog integer".into())),
        other => Err(Error::Protocol(format!(
            "expected catalog integer, got {other:?}"
        ))),
    }
}

fn optional_unsigned_value(value: Value) -> Result<Option<u32>> {
    match value {
        Value::Null => Ok(None),
        other => unsigned_value(other).map(Some),
    }
}
