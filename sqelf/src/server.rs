use std::net::SocketAddr;

use tokio::{
    codec::Decoder,
    net::udp::{UdpFramed, UdpSocket},
    prelude::*,
};

use bytes::{Bytes, BytesMut};

use futures::{future::lazy, sync::mpsc};

pub type Error = failure::Error;

use crate::receive::Message;
use crate::diagnostics::emit_err;

/**
Server configuration.
*/
#[derive(Debug, Clone)]
pub struct Config {
    /**
    The address to bind the UDP server to.
    */
    pub bind: String,
    /**
    The maximum number of unprocessed messages.

    If this value is reached then incoming messages will be dropped.
    */
    pub unprocessed_capacity: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bind: "0.0.0.0:12201".to_owned(),
            unprocessed_capacity: 1024,
        }
    }
}

/**
Build a server to receive GELF messages and process them.
*/
pub fn build(
    config: Config,
    receive: impl FnMut(Bytes) -> Result<Option<Message>, Error> + Send + Sync + 'static,
    mut handle: impl FnMut(Message) -> Result<(), Error> + Send + Sync + 'static,
) -> Result<impl Future<Item = (), Error = ()>, Error> {
    let addr: SocketAddr = config.bind.parse()?;
    let sock = UdpSocket::bind(&addr)?;

    let (tx, rx) = mpsc::channel(config.unprocessed_capacity);

    Ok(lazy(move || {
        // Spawn a background task to process events
        tokio::spawn(lazy(move || {
            rx.for_each(move |msg| {
                handle(msg).or_else(|e: Error| {
                    emit_err(&e, "GELF processing failed");

                    Ok(())
                })
            })
        }));

        // Accept and process incoming GELF messages over UDP
        UdpFramed::new(sock, Decode(receive))
            .for_each(move |(msg, _)| {
                let tx = tx.clone();

                tx.send(msg).map(|_| ()).or_else(|e| {
                    emit_err(&e, "GELF buffering failed");

                    Ok(())
                })
            })
            .or_else(|e| {
                emit_err(&e, "GELF receive failed");

                Ok(())
            })
    }))
}

struct Decode<F>(F);

impl<F> Decoder for Decode<F>
where
    F: FnMut(Bytes) -> Result<Option<Message>, Error>,
{
    type Item = Message;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let src = src.take().freeze();

        (self.0)(src)
    }
}
