use std::net::SocketAddr;

use tokio::{
    codec::Decoder,
    net::udp::{UdpFramed, UdpSocket},
    prelude::*,
};

use bytes::{Bytes, BytesMut};

use futures::{future::lazy, sync::mpsc};

pub type Error = failure::Error;

use crate::diagnostics::*;
use crate::receive::Message;

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

    let shutdown = tokio_signal::ctrl_c().map_err(emit_abort("Server setup failed"));

    Ok(shutdown.and_then(move |shutdown| {
        // Spawn a background task to process events
        tokio::spawn(lazy(move || {
            rx.for_each(move |msg| handle(msg).or_else(emit_continue("GELF processing failed")))
        }));

        // Listen for Ctrl + C and other termination signals
        // from the OS
        let shutdown = shutdown
            .map(|_| Op::Shutdown)
            .map_err(emit_abort("Server shutdown was unclean"));

        // Accept and process incoming GELF messages over UDP
        // This stream should never return an `Err` variant
        let server = UdpFramed::new(sock, Decode(receive))
            .map(|(msg, _)| Op::Receive(Some(msg)))
            .or_else(emit_continue_with("GELF receive failed", receive_empty));

        server
            .select(shutdown)
            .and_then(|msg| match msg {
                // Continue processing received messages
                Op::Receive(msg) => Ok(msg),
                // Terminate on shutdown messages
                // The error here causes the future to return
                Op::Shutdown => {
                    emit("Termination signal received; shutting down");

                    Err(())
                }
            })
            .filter_map(|msg| msg)
            .for_each(move |msg| {
                let tx = tx.clone();
                tx.send(msg)
                    .map(|_| ())
                    .or_else(emit_continue("GELF buffering failed"))
            })
    }))
}

struct Decode<F>(F);

impl<F> Decoder for Decode<F>
where
    F: FnMut(Bytes) -> Result<Option<Message>, Error>,
{
    type Item = Gelf;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let src = src.take().freeze();

        (self.0)(src)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Op {
    Receive(Option<Message>),
    Shutdown,
}

fn receive_empty() -> Op {
    Op::Receive(None)
}
