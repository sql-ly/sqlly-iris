use std::{fmt, time::Duration};

use percent_encoding::percent_decode_str;
use zeroize::Zeroizing;

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Encryption {
    #[default]
    Disabled,
    Required,
}

pub struct Config {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) namespace: String,
    pub(crate) username: String,
    pub(crate) password: Zeroizing<String>,
    pub(crate) application_name: String,
    pub(crate) machine_name: String,
    pub(crate) query_timeout: Duration,
    pub(crate) connect_timeout: Duration,
    pub(crate) max_rows: u32,
    pub(crate) encryption: Encryption,
}

impl Config {
    pub fn new(
        host: impl Into<String>,
        namespace: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            host: host.into(),
            port: 1972,
            namespace: namespace.into(),
            username: username.into(),
            password: Zeroizing::new(password.into()),
            application_name: "sqlly-iris".into(),
            machine_name: "rust".into(),
            query_timeout: Duration::ZERO,
            connect_timeout: Duration::from_secs(10),
            max_rows: 0,
            encryption: Encryption::Disabled,
        }
    }

    pub fn from_url(input: &str) -> Result<Self> {
        let url = url::Url::parse(input)
            .map_err(|error| Error::InvalidConfig(format!("invalid connection URL: {error}")))?;
        if url.scheme() != "iris" {
            return Err(Error::InvalidConfig(
                "connection URL scheme must be iris://".into(),
            ));
        }
        let host = url
            .host_str()
            .ok_or_else(|| Error::InvalidConfig("connection URL has no host".into()))?;
        let namespace = url.path().trim_start_matches('/');
        if namespace.is_empty() || namespace.contains('/') {
            return Err(Error::InvalidConfig(
                "connection URL must contain exactly one namespace".into(),
            ));
        }
        let namespace = percent_decode_str(namespace)
            .decode_utf8()
            .map_err(|_| Error::InvalidConfig("namespace is not valid UTF-8".into()))?;
        let password = percent_decode_str(url.password().unwrap_or_default())
            .decode_utf8()
            .map_err(|_| Error::InvalidConfig("password is not valid UTF-8".into()))?;
        let username = percent_decode_str(url.username())
            .decode_utf8()
            .map_err(|_| Error::InvalidConfig("username is not valid UTF-8".into()))?;
        let mut config = Self::new(host, namespace, username, password);
        if let Some(port) = url.port() {
            config.port = port;
        }
        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "query_timeout" => {
                    config.query_timeout = Duration::from_secs(parse_seconds(&key, &value)?);
                }
                "connect_timeout" => {
                    config.connect_timeout = Duration::from_secs(parse_seconds(&key, &value)?);
                }
                "max_rows" => {
                    config.max_rows = value.parse().map_err(|_| {
                        Error::InvalidConfig("max_rows must be an unsigned integer".into())
                    })?;
                }
                "application_name" => config.application_name = value.into_owned(),
                "encryption" => {
                    config.encryption = match value.as_ref() {
                        "disable" | "disabled" => Encryption::Disabled,
                        "require" | "required" => Encryption::Required,
                        _ => {
                            return Err(Error::InvalidConfig(
                                "encryption must be disable or require".into(),
                            ));
                        }
                    };
                }
                _ => {
                    return Err(Error::InvalidConfig(format!(
                        "unknown connection URL option {key:?}"
                    )));
                }
            }
        }
        config.validate()?;
        Ok(config)
    }

    pub fn port(&mut self, port: u16) -> &mut Self {
        self.port = port;
        self
    }

    pub fn application_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.application_name = name.into();
        self
    }

    pub fn machine_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.machine_name = name.into();
        self
    }

    pub fn query_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.query_timeout = timeout;
        self
    }

    pub fn connect_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn max_rows(&mut self, rows: u32) -> &mut Self {
        self.max_rows = rows;
        self
    }

    pub fn encryption(&mut self, encryption: Encryption) -> &mut Self {
        self.encryption = encryption;
        self
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.host.is_empty() {
            return Err(Error::InvalidConfig("host cannot be empty".into()));
        }
        if self.namespace.is_empty() {
            return Err(Error::InvalidConfig("namespace cannot be empty".into()));
        }
        if self.username.is_empty() {
            return Err(Error::InvalidConfig("username cannot be empty".into()));
        }
        if self.query_timeout.as_secs() > i32::MAX as u64 {
            return Err(Error::InvalidConfig("query timeout is too large".into()));
        }
        for (name, value) in [
            ("namespace", self.namespace.as_str()),
            ("username", self.username.as_str()),
            ("password", self.password.as_str()),
            ("application name", self.application_name.as_str()),
            ("machine name", self.machine_name.as_str()),
        ] {
            if crate::protocol::encoded_text_len(value, false) > crate::protocol::MAX_LIST_DATA_LEN
            {
                return Err(Error::InvalidConfig(format!(
                    "{name} exceeds the IRIS string limit"
                )));
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Config")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("namespace", &self.namespace)
            .field("username", &self.username)
            .field("password", &"[redacted]")
            .field("application_name", &self.application_name)
            .field("machine_name", &self.machine_name)
            .field("query_timeout", &self.query_timeout)
            .field("connect_timeout", &self.connect_timeout)
            .field("max_rows", &self.max_rows)
            .field("encryption", &self.encryption)
            .finish()
    }
}

fn parse_seconds(key: &str, value: &str) -> Result<u64> {
    value
        .parse()
        .map_err(|_| Error::InvalidConfig(format!("{key} must be an unsigned number of seconds")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_redacts_connection_urls() {
        let config = Config::from_url(
            "iris://_SYSTEM:p%40ss@localhost:21972/USER?query_timeout=30&max_rows=100",
        )
        .unwrap();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 21972);
        assert_eq!(config.namespace, "USER");
        assert_eq!(config.username, "_SYSTEM");
        assert_eq!(&*config.password, "p@ss");
        assert_eq!(config.query_timeout, Duration::from_secs(30));
        assert_eq!(config.max_rows, 100);
        let debug = format!("{config:?}");
        assert!(!debug.contains("p@ss"));
        assert!(debug.contains("[redacted]"));
    }

    #[test]
    fn rejects_unknown_options() {
        let error = Config::from_url("iris://u:p@localhost/USER?encrption=require").unwrap_err();
        assert!(error.to_string().contains("unknown connection URL option"));
    }
}
