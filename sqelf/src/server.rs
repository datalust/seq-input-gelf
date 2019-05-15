use std::{
    net::SocketAddr,
    thread,
};

use tokio::{
    prelude::*,
    runtime::Runtime,
};

use bytes::{Bytes, BytesMut};

use futures::{
    future::lazy,
    future::Either,
    sync::{mpsc, oneshot},
};

use crate::{
    diagnostics::*,
    error::{err_msg, Error},
    receive::Message,
};

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
    /**
    Whether or not to set up a handle that can be used to control the server
    within the same process.
    */
    pub bind_handle: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bind: "0.0.0.0:12201".to_owned(),
            unprocessed_capacity: 1024,
            wait_on_stdin: false,
            bind_handle: false,
        }
    }
}

/**
A GELF server.
*/
pub struct Server(Box<dyn Future<Item = (), Error = Exit> + Send>);

impl Server {
    pub fn run(self) -> Result<(), Error> {
        // Run the server on a fresh runtime
        // We attempt to shut this runtime down cleanly to release
        // any used resources
        let mut runtime = Runtime::new().expect("failed to start new Runtime");

        let run_server = {
            match runtime.block_on(self.0) {
                Ok(()) | Err(Exit::Clean) => Ok(()),
                _ => Err(err_msg("Server execution failed").into()),
            }
        };

        let close_runtime = runtime
            .shutdown_now()
            .wait()
            .map_err(|_| err_msg("Runtime shutdown failed"));

        run_server.and(close_runtime)
    }
}

/**
A handle to a running GELF server that can be used to interact with it
programmatically.
*/
pub struct Handle {
    close: oneshot::Sender<()>,
}

impl Handle {
    /**
    Close the server.
    */
    pub fn close(self) -> bool {
        self.close.send(()).is_ok()
    }
}

/**
Build a server to receive GELF messages and process them.

If `config.bind_handle` is `true`, then this function will return a handle
that can be used to interact with the running server programmatically.
*/
pub fn build(
    config: Config,
    receive: impl FnMut(Bytes) -> Result<Option<Message>, Error> + Send + Sync + 'static,
    mut process: impl FnMut(Message) -> Result<(), Error> + Send + Sync + 'static,
) -> Result<(Server, Option<Handle>), Error> {
    emit("Starting GELF server");

    let addr = config.bind.parse()?;
    let sock = ServerSocket::bind(&addr)?;

    let (process_tx, process_rx) = mpsc::channel(config.unprocessed_capacity);
    let (handle_tx, handle_rx) = oneshot::channel();

    // Build a handle
    let handle = if config.bind_handle {
        Some(Handle { close: handle_tx })
    } else {
        None
    };

    // Attempt to bind shutdown signals
    let ctrl_c =
        tokio_signal::ctrl_c().map_err(emit_err_abort_with("Server setup failed", exit_failure));

    let server = ctrl_c.and_then(move |ctrl_c| {
        // Spawn a background task to process GELF payloads
        let process = tokio::spawn(lazy(move || {
            process_rx.for_each(move |msg| match process(msg) {
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

        // Maybe listen for `stdin` closing
        let stdin_closed = if config.wait_on_stdin {
            Either::A(stdin_closed().map(|_| Op::Shutdown))
        } else {
            Either::B(future::empty())
        }
        .into_stream();

        // Maybe listen for a programmatic handle closing
        let handle_closed = if config.bind_handle {
            Either::A(handle_rx.then(|_| Ok(Op::Shutdown)))
        } else {
            Either::B(future::empty())
        }
        .into_stream();

        // Listen for Ctrl + C and other termination signals
        // from the OS
        let ctrl_c = ctrl_c
            .map(|_| Op::Shutdown)
            .map_err(emit_err_abort("Server shutdown was unclean"));

        let shutdown = ctrl_c
            .select(stdin_closed)
            .select(handle_closed);

        // Accept and process incoming GELF messages over UDP
        // This stream should never return an `Err` variant
        let receive = sock.build(receive).then(|r| match r {
            Ok(msg) => {
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
                process_tx
                    .clone()
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
    });

    Ok((Server(Box::new(server)), handle))
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

mod imp {
    use super::*;

    use tokio::{
        codec::Decoder,
        net::udp::{UdpFramed, UdpSocket},
    };

    pub(super) struct ServerSocket(UdpSocket);

    impl ServerSocket {
        pub(super) fn bind(addr: &SocketAddr) -> Result<Self, Error> {
            let sock = UdpSocket::bind(&addr)?;
            
            Ok(ServerSocket(sock))
        }

        pub(super) fn build(self, receive: impl FnMut(Bytes) -> Result<Option<Message>, Error> + Send + Sync + 'static) -> impl Stream<Item = Message, Error = Error> {
            UdpFramed::new(self.0, Decode(receive)).map(|(msg, _)| msg)
        }
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
}

use self::imp::*;
