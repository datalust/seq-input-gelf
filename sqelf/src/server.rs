use std::thread;

use tokio::{
    codec::Decoder,
    net::udp::{UdpFramed, UdpSocket},
    prelude::*,
};

use bytes::{Bytes, BytesMut};

use futures::{future::lazy, future::Either, sync::mpsc};

use crate::{diagnostics::*, error::Error, receive::Message};

metrics! {
    receive_ok,
    receive_err,
    process_ok,
    process_err
}

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
    let addr = config.bind.parse()?;
    let sock = UdpSocket::bind(&addr)?;
    let (tx, rx) = mpsc::channel(config.unprocessed_capacity);

    // Attempt to bind shutdown signals
    let ctrl_c =
        tokio_signal::ctrl_c().map_err(emit_err_abort_with("Server setup failed", exit_failure));

    Ok(ctrl_c.and_then(move |ctrl_c| {
        // Spawn a background task to process GELF payloads
        let process = tokio::spawn(lazy(move || {
            rx.for_each(move |msg| match handle(msg) {
                Ok(()) => {
                    increment!(server.process_ok);

                    Ok(())
                }
                Err(err) => {
                    increment!(server.process_err);
                    emit_err(&err, "GELF processing failed");

                    Ok(())
                }
            })
        }));

        // Spawn a background task to poll `stdio`
        let stdin_closed = if config.wait_on_stdin {
            Either::A(stdin_closed().map(|_| Op::Shutdown))
        } else {
            Either::B(future::empty())
        }
        .into_stream();

        // Listen for Ctrl + C and other termination signals
        // from the OS
        let ctrl_c = ctrl_c
            .map(|_| Op::Shutdown)
            .map_err(emit_err_abort("Server shutdown was unclean"));

        // Trigger shutdown on Ctrl + C or when stdin is closed
        let shutdown = ctrl_c.select(stdin_closed);

        // Accept and process incoming GELF messages over UDP
        // This stream should never return an `Err` variant
        let receive = UdpFramed::new(sock, Decode(receive)).then(|r| match r {
            Ok((msg, _)) => {
                increment!(server.receive_ok);

                Ok(Op::Receive(Some(msg)))
            }
            Err(err) => {
                increment!(server.receive_err);
                emit_err(&err, "GELF receiving failed");

                Ok(Op::Receive(None))
            }
        });

        // Stich the UDP messages with shutdown messages
        // Triggering an error here will cause the stream
        // to terminate
        let server = receive
            .select(shutdown)
            .and_then(|msg| match msg {
                // Continue processing received messages
                // Errors from receiving will be surfaced
                // here as `Ok(None)`
                Op::Receive(msg) => Ok(msg),
                // Terminate on shutdown messages
                // The error here causes the future to return
                Op::Shutdown => {
                    emit("Termination signal received; shutting down");

                    Err(())
                }
            })
            .filter_map(|msg| msg);

        // For each message, send it to the background for processing
        // Since processing is synchronous we don't want to hold up
        // the works here waiting for it.
        server
            .for_each(move |msg| {
                tx.clone()
                    .send(msg)
                    .map(|_| ())
                    .or_else(emit_err_continue("GELF buffering failed"))
            })
            // The `for_each` call won't return until either the
            // UDP server is closed (which shouldn't happen) or
            // a termination signal breaks the loop.
            //
            // If we get this far then the server is shutting down
            // Wait for the message pipeline to terminate.
            .then(|_| {
                emit("Waiting for messages to finish processing");

                process
            })
            // Forces the runtime to shutdown
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

impl<F> Drop for Decode<F> {
    fn drop(&mut self) {
        emit("Dropping the UDP decoder");
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Op {
    Receive(Option<Message>),
    Shutdown,
}

fn stdin_closed() -> impl Future<Item = (), Error = ()> {
    // NOTE: This is a regular thread instead of `tokio`
    // so that we don't block with our synchronous read that
    // will probably never actually return
    let (tx, rx) = mpsc::channel(1);
    thread::spawn(move || 'wait: loop {
        match std::io::stdin().read(&mut [u8::default()]) {
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
