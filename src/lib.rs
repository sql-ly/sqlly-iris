#![forbid(unsafe_code)]

mod client;
mod config;
mod error;
mod metadata;
mod protocol;
mod value;

pub use client::{Client, ExecuteResult, QueryStream, ServerInfo, Transaction};
pub use config::{Config, Encryption};
pub use error::{Error, Result};
pub use metadata::{Column, Nullable, Row, SqlType};
pub use value::Value;

pub const PROTOCOL_VERSION: u16 = 69;
