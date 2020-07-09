use std::{
    marker::Unpin,
    str::FromStr,
    time::Duration,
};

use futures::{
    future::{
        BoxFuture,
        Either,
    },
    select,
    FutureExt,
    StreamExt,
};

use tokio::{
    runtime::Runtime,
    signal::ctrl_c,
    sync::oneshot,
};

use bytes::Bytes;

use crate::{
    diagnostics::*,
    error::Error,
    receive::Message,
};

mod tcp;
mod udp;

metrics! {
    receive_ok,
    receive_err,
    process_ok,
    process_err,
    tcp_conn_accept,
    tcp_conn_close,
    tcp_conn_timeout,
    tcp_msg_overflow
}

/**
Server configuration.
*/
#[derive(Debug, Clone)]
pub struct Config {
    /**
    The address to bind the server to.
    */
    pub bind: Bind,
    /**
    The duration to keep client TCP connections alive for.

    If the client doesn't complete a message within the period
    then the connection will be closed.
    */
    pub tcp_keep_alive_secs: u64,
    /**
    The maximum size of a single event before it'll be discarded.
    */
    pub tcp_max_size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct Bind {
    pub addr: String,
    pub protocol: Protocol,
}

#[derive(Debug, Clone, Copy)]
pub enum Protocol {
    Udp,
    Tcp,
}

impl FromStr for Bind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.get(0..6) {
            Some("tcp://") => Ok(Bind {
                addr: s[6..].to_owned(),
                protocol: Protocol::Tcp,
            }),
            Some("udp://") => Ok(Bind {
                addr: s[6..].to_owned(),
                protocol: Protocol::Udp,
            }),
            _ => Ok(Bind {
                addr: s.to_owned(),
                protocol: Protocol::Udp,
            }),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bind: Bind {
                addr: "0.0.0.0:12201".to_owned(),
                protocol: Protocol::Udp,
            },
            tcp_keep_alive_secs: 2 * 60,    // 2 minutes
            tcp_max_size_bytes: 1024 * 256, // 256kiB
        }
    }
}

/**
A GELF server.
*/
pub struct Server {
    fut: BoxFuture<'static, ()>,
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

        runtime.block_on(self.fut);

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
    receive: impl FnMut(Bytes) -> Result<Option<Message>, Error> + Send + Sync + Unpin + Clone + 'static,
    mut process: impl FnMut(Message) -> Result<(), Error> + Send + Sync + Unpin + Clone + 'static,
) -> Result<Server, Error> {
    emit("Starting GELF server");

    let addr = config.bind.addr.parse()?;
    let (handle_tx, handle_rx) = oneshot::channel();

    // Build a handle
    let handle = Some(Handle { close: handle_tx });

    let server = async move {
        let incoming = match config.bind.protocol {
            Protocol::Udp => {
                let server = udp::Server::bind(&addr).await?.build(receive);

                Either::Left(server)
            }
            Protocol::Tcp => {
                let server = tcp::Server::bind(&addr).await?.build(
                    Duration::from_secs(config.tcp_keep_alive_secs),
                    config.tcp_max_size_bytes as usize,
                    receive,
                );

                Either::Right(server)
            }
        };

        let mut close = handle_rx.fuse();
        let mut ctrl_c = ctrl_c().boxed().fuse();
        let mut incoming = incoming.fuse();

        // NOTE: We don't use `?` here because we never want to carry results
        // We always want to match them and deal with error cases directly
        loop {
            select! {
                // A message that's ready to process
                msg = incoming.next() => match msg {
                    // A complete message has been received
                    Some(Ok(Received::Complete(msg))) => {
                        increment!(server.receive_ok);

                        // Process the received message
                        match process(msg) {
                            Ok(()) => {
                                increment!(server.process_ok);
                            }
                            Err(err) => {
                                increment!(server.process_err);
                                emit_err(&err, "GELF processing failed");
                            }
                        }
                    },
                    // A chunk of a message has been received
                    Some(Ok(Received::Incomplete)) => {
                        continue;
                    },
                    // An error occurred receiving a chunk
                    Some(Err(err)) => {
                        increment!(server.receive_err);
                        emit_err(&err, "GELF processing failed");
                    },
                    None => {
                        unreachable!("receiver stream should never terminate")
                    },
                },
                // A termination signal from the programmatic handle
                _ = close => {
                    emit("Handle closed; shutting down");
                    break;
                },
                // A termination signal from the environment
                _ = ctrl_c => {
                    emit("Termination signal received; shutting down");
                    break;
                },
            };
        }

        emit("Stopping GELF server");

        Result::Ok::<(), Error>(())
    };

    Ok(Server {
        fut: Box::pin(async move {
            if let Err(err) = server.await {
                emit_err(&err, "GELF server failed");
            }
        }),
        handle,
    })
}

enum Received {
    Incomplete,
    Complete(Message),
}

trait OptionMessageExt {
    fn into_received(self) -> Option<Received>;
}

impl OptionMessageExt for Option<Message> {
    fn into_received(self) -> Option<Received> {
        match self {
            Some(msg) => Some(Received::Complete(msg)),
            None => Some(Received::Incomplete),
        }
    }
}
