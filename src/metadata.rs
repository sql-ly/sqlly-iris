use std::sync::Arc;

use crate::{Error, Result, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlType {
    Guid,
    WLongVarChar,
    WVarChar,
    WChar,
    Bit,
    TinyInt,
    BigInt,
    LongVarBinary,
    VarBinary,
    Binary,
    LongVarChar,
    Char,
    Numeric,
    Decimal,
    Integer,
    SmallInt,
    Float,
    Real,
    Double,
    Date,
    Time,
    Timestamp,
    VarChar,
    DateHorolog,
    TimeHorolog,
    TimestampPosix,
    Unknown(i16),
}

impl SqlType {
    pub(crate) fn from_code(code: i16) -> Self {
        match code {
            -11 => Self::Guid,
            -10 => Self::WLongVarChar,
            -9 => Self::WVarChar,
            -8 => Self::WChar,
            -7 => Self::Bit,
            -6 => Self::TinyInt,
            -5 => Self::BigInt,
            -4 => Self::LongVarBinary,
            -3 => Self::VarBinary,
            -2 => Self::Binary,
            -1 => Self::LongVarChar,
            1 => Self::Char,
            2 => Self::Numeric,
            3 => Self::Decimal,
            4 => Self::Integer,
            5 => Self::SmallInt,
            6 => Self::Float,
            7 => Self::Real,
            8 => Self::Double,
            9 | 91 => Self::Date,
            10 | 92 => Self::Time,
            11 | 93 => Self::Timestamp,
            12 => Self::VarChar,
            1091 => Self::DateHorolog,
            1092 => Self::TimeHorolog,
            1093 => Self::TimestampPosix,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nullable {
    NoNulls,
    Nullable,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub sql_type: SqlType,
    pub precision: u32,
    pub scale: i32,
    pub nullable: Nullable,
    pub label: String,
    pub table_name: String,
    pub schema: String,
    pub catalog: String,
    pub is_auto_increment: bool,
    pub is_case_sensitive: bool,
    pub is_currency: bool,
    pub is_read_only: bool,
    pub is_row_id: bool,
    pub(crate) slot_position: usize,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub(crate) columns: Arc<Vec<Column>>,
    pub(crate) values: Vec<Value>,
}

impl Row {
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    pub fn values(&self) -> &[Value] {
        &self.values
    }

    pub fn get(&self, index: usize) -> Result<&Value> {
        self.values.get(index).ok_or(Error::ColumnIndex(index))
    }

    pub fn get_by_name(&self, name: &str) -> Result<&Value> {
        let index = self
            .columns
            .iter()
            .position(|column| column.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| Error::ColumnName(name.into()))?;
        self.get(index)
    }

    pub fn into_values(self) -> Vec<Value> {
        self.values
    }
}
