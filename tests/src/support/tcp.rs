use crate::support::SERVER_HOST;
use std::convert::TryInto;
use std::sync::Arc;
use std::{
    io::Write,
    net::{
        self,
        TcpStream,
    },
};

use super::SERVER_ADDR;

pub struct Stream {
    inner: TcpStream,
}

pub fn stream() -> Stream {
    Stream::new()
}

impl Stream {
    fn new() -> Self {
        let stream = TcpStream::connect(SERVER_ADDR).expect("failed to bind client stream");

        Stream { inner: stream }
    }

    pub fn write(&mut self, chunks: Vec<Vec<u8>>) {
        for chunk in chunks {
            self.inner.write(&chunk).expect("failed to send chunk");
        }
    }

    pub fn close(self) {
        let _ = self.inner.shutdown(net::Shutdown::Both);
    }
}

pub struct TlsStream {
    inner: rustls::StreamOwned<rustls::ClientConnection, TcpStream>,
}

pub fn tls_stream() -> TlsStream {
    TlsStream::new()
}

impl TlsStream {
    fn new() -> Self {
        let root_store = {
            let mut root_store = rustls::RootCertStore::empty();

            for cert in rustls_native_certs::load_native_certs()
                .expect("failed to read native certificates")
            {
                root_store
                    .add(&rustls::Certificate(cert.0))
                    .expect("failed to add certificate");
            }

            root_store
        };

        let config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        let connection = rustls::ClientConnection::new(
            Arc::new(config),
            SERVER_HOST
                .try_into()
                .expect("failed to convert the server host into a server name"),
        )
        .expect("failed to initiate connection");

        let stream = TcpStream::connect(SERVER_ADDR).unwrap();

        TlsStream {
            inner: rustls::StreamOwned::new(connection, stream),
        }
    }

    pub fn write(&mut self, chunks: Vec<Vec<u8>>) {
        for chunk in chunks {
            self.inner.write(&chunk).expect("failed to send chunk");
        }

        self.inner.flush().expect("failed to flush chunk");
    }

    pub fn close(self) {
        let _ = self.inner.sock.shutdown(net::Shutdown::Both);
    }
}
