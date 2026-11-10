#![forbid(unsafe_code)]

mod catalog;
mod client;
mod config;
mod convert;
mod error;
mod metadata;
mod native;
mod procedure;
mod protocol;
mod session;
mod statement;
mod stream;
mod transport;
mod value;

pub use catalog::{
    Column as CatalogColumn, Procedure as CatalogProcedure, Schema as CatalogSchema,
    Table as CatalogTable,
};
pub use client::{Client, ExecuteResult, QueryStream, ServerInfo, Transaction};
pub use config::{Config, Encryption};
pub use error::{Error, Result};
pub use metadata::{Column, Nullable, Row, SqlType};
pub use native::{GlobalData, GlobalDirection};
pub use procedure::{ProcedureCall, ProcedureResult};
pub use session::IsolationLevel;
pub use statement::PreparedStatement;
pub use value::Value;

pub const PROTOCOL_VERSION: u16 = 69;
