use std::sync::Arc;

use rustls::{ClientConfig, RootCertStore};
use rustls_pki_types::{CertificateDer, ServerName, pem::PemObject};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsConnector;

use crate::{Config, Encryption, Error, Result};

pub(crate) trait IoStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T> IoStream for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

pub(crate) type BoxedStream = Box<dyn IoStream>;

pub(crate) async fn establish(config: &Config, tcp: tokio::net::TcpStream) -> Result<BoxedStream> {
    if config.encryption == Encryption::Disabled {
        return Ok(Box::new(tcp));
    }
    let mut roots = RootCertStore::empty();
    let native = rustls_native_certs::load_native_certs();
    for certificate in native.certs {
        roots.add(certificate).map_err(|error| {
            Error::Tls(format!("failed to add a native CA certificate: {error}"))
        })?;
    }
    if roots.is_empty() {
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }
    if let Some(path) = &config.ca_certificate {
        let pem = std::fs::read(path).map_err(|error| {
            Error::Tls(format!(
                "failed to read CA certificate {}: {error}",
                path.display()
            ))
        })?;
        for certificate in CertificateDer::pem_slice_iter(&pem) {
            let certificate = certificate.map_err(|error| {
                Error::Tls(format!(
                    "invalid CA certificate {}: {error}",
                    path.display()
                ))
            })?;
            roots.add(certificate).map_err(|error| {
                Error::Tls(format!(
                    "failed to add CA certificate {}: {error}",
                    path.display()
                ))
            })?;
        }
    }
    let tls_config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(tls_config));
    let server_name = ServerName::try_from(config.host.clone())
        .map_err(|error| Error::Tls(format!("invalid TLS server name {}: {error}", config.host)))?;
    let stream = connector.connect(server_name, tcp).await.map_err(|error| {
        Error::Tls(format!(
            "TLS handshake with {} failed: {error}",
            config.host
        ))
    })?;
    Ok(Box::new(stream))
}
