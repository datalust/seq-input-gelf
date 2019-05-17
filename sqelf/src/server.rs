use std::net::SocketAddr;

use tokio::{prelude::*, runtime::Runtime};

use bytes::{Bytes, BytesMut};

use futures::{
    future::lazy,
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
A GELF server.
*/
pub struct Server {
    fut: Box<dyn Future<Item = (), Error = ()> + Send>,
    handle: Option<Handle>,
}

impl Server {
    pub fn take_handle(&mut self) -> Option<Handle> {
        self.handle.take()
    }

    pub fn run(self) -> Result<(), Error> {
        // Run the server on a fresh runtime
        // We attempt to shut this runtime down cleanly to release
        // any used resources
        let mut runtime = Runtime::new().expect("failed to start new Runtime");

        runtime
            .block_on(self.fut)
            .map_err(|_| err_msg("Server execution failed"))?;

        runtime
            .shutdown_now()
            .wait()
            .map_err(|_| err_msg("Runtime shutdown failed"))?;

        Ok(())
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
*/
pub fn build(
    config: Config,
    receive: impl FnMut(Bytes) -> Result<Option<Message>, Error> + Send + Sync + 'static,
    mut process: impl FnMut(Message) -> Result<(), Error> + Send + Sync + 'static,
) -> Result<Server, Error> {
    emit("Starting GELF server");

    let addr = config.bind.parse()?;
    let sock = ServerSocket::bind(&addr)?;

    let (process_tx, process_rx) = mpsc::channel(config.unprocessed_capacity);
    let (handle_tx, handle_rx) = oneshot::channel();

    // Build a handle
    let handle = Some(Handle { close: handle_tx });

    // Attempt to bind shutdown signals
    let ctrl_c = tokio_signal::ctrl_c().map_err(emit_err_abort("Server setup failed"));

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

        // Listen for a programmatic handle closing
        let handle_closed = handle_rx.then(|_| Ok(Op::Shutdown)).into_stream();

        // Listen for Ctrl + C and other termination signals
        // from the OS
        let ctrl_c = ctrl_c
            .map(|_| Op::Shutdown)
            .map_err(emit_err_abort("Server shutdown was unclean"));

        let shutdown = ctrl_c.select(handle_closed);

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
    });

    Ok(Server {
        fut: Box::new(server),
        handle,
    })
}

#[derive(Debug, PartialEq, Eq)]
enum Op {
    Receive(Option<Message>),
    Shutdown,
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

        pub(super) fn build(
            self,
            receive: impl FnMut(Bytes) -> Result<Option<Message>, Error> + Send + Sync + 'static,
        ) -> impl Stream<Item = Message, Error = Error> {
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
