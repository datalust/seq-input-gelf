use std::{net::SocketAddr, thread};

use tokio::{
    codec::Decoder,
    net::udp::{UdpFramed, UdpSocket},
    prelude::*,
};

use bytes::{Bytes, BytesMut};

use futures::{future::lazy, sync::mpsc, future::Either};

use crate::{
    error::Error,
    diagnostics::*,
    receive::Message,
};

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

    /**
    Whether or not the server should wait on (and terminate on the completion of)
    the process's standard input.

    This is used by Seq on Windows to signal to a
    background process - which has no window to receive WM_CLOSE, and no console
    to receive Ctrl+C, that the process should exit.
    */
    pub wait_on_stdin: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bind: "0.0.0.0:12201".to_owned(),
            unprocessed_capacity: 1024,
            wait_on_stdin: false,
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
) -> Result<impl Future<Item = (), Error = Exit>, Error> {
    let addr: SocketAddr = config.bind.parse()?;
    let sock = UdpSocket::bind(&addr)?;

    let (tx, rx) = mpsc::channel(config.unprocessed_capacity);

    // Attempt to bind shutdown signals
    let shutdown = tokio_signal::ctrl_c().map_err(emit_abort_with("Server setup failed", exit_failure));

    Ok(shutdown.and_then(move |shutdown| {
        // Spawn a background task to process GELF payloads
        let process = tokio::spawn(lazy(move || {
            rx.for_each(move |msg| handle(msg).or_else(emit_continue("GELF processing failed")))
        }));

        // Spawn a background task to poll `stdio`
        let stdin_closed = if config.wait_on_stdin {
            Either::A(stdin_closed()
                .map(|_| Op::Shutdown))
        } else {
            Either::B(future::empty())
        }.into_stream();

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
            .select(stdin_closed)
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
            // Process messages
            .filter_map(|msg| msg)
            .for_each(move |msg| {
                let tx = tx.clone();
                tx.send(msg)
                    .map(|_| ())
                    .or_else(emit_continue("GELF buffering failed"))
            })
            // If we get this far then the server is shutting down
            // Wait for the message pipeline to terminate
            .then(|_| process)
            // FIXME: Forces the runtime to shutdown
            // This is a bit of a hack that prevents
            // `tokio` from waiting on any remaining futures
            // since we're terminating the process
            .then(|r| match r {
                Ok(()) => Err(Exit::Clean),
                Err(()) => Err(Exit::Failure),
            })
    }))
}

/**
The outcome of shutting down the server.
*/
pub enum Exit {
    /**
    The server was terminated, but was done so cleanly.
    */
    Clean,
    /**
    The server was terminated, but due to an internal error.
    */
    Failure,
}

fn exit_failure() -> Exit {
    Exit::Failure
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

#[derive(Debug, PartialEq, Eq)]
enum Op {
    Receive(Option<Message>),
    Shutdown,
}

fn receive_empty() -> Op {
    Op::Receive(None)
}

fn stdin_closed() -> impl Future<Item = (), Error = ()> {
    let (tx, rx) = mpsc::channel(1);

    thread::spawn(move || 'wait: loop {
        match std::io::stdin().read(&mut [0]) {
            Ok(0) => {
                let _ = tx.send(()).wait();
                break 'wait;
            }
            Ok(_) => {
                continue 'wait;
            }
            Err(_) => {
                let _ = tx.send(()).wait();
                break 'wait;
            }
        }
    });

    rx.into_future().map(|_| ()).map_err(|_| ())
}
