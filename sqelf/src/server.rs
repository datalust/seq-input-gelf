use std::{net::SocketAddr, str::FromStr, time::Duration};

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
    process_err,
    tcp_conn_accept,
    tcp_conn_close,
    tcp_conn_timeout
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
    The maximum number of unprocessed messages.

    If this value is reached then incoming messages will be dropped.
    */
    pub unprocessed_capacity: usize,
    /**
    The duration to keep client TCP connections alive for.

    If the client doesn't complete a message within the period
    then the connection will be closed.
    */
    pub tcp_keep_alive_secs: u64,
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
            })
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
            unprocessed_capacity: 1024,
            tcp_keep_alive_secs: 2 * 60, // 2 minutes
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
    receive: impl FnMut(Bytes) -> Result<Option<Message>, Error> + Send + Sync + 'static + Clone,
    mut process: impl FnMut(Message) -> Result<(), Error> + Send + Sync + 'static + Clone,
) -> Result<Server, Error> {
    emit("Starting GELF server");

    let addr = config.bind.addr.parse()?;

    let incoming: Box<dyn Stream<Item = Message, Error = Error> + Send + Sync> =
        match config.bind.protocol {
            Protocol::Udp => Box::new(udp::Server::bind(&addr)?.build(receive)),
            Protocol::Tcp => Box::new(
                tcp::Server::bind(&addr)?
                    .build(Duration::from_secs(config.tcp_keep_alive_secs), receive),
            ),
        };

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
        let receive = incoming.then(|r| match r {
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

mod udp {
    use super::*;

    use tokio::{
        codec::Decoder,
        net::udp::{UdpFramed, UdpSocket},
    };

    pub(super) struct Server(UdpSocket);

    impl Server {
        pub(super) fn bind(addr: &SocketAddr) -> Result<Self, Error> {
            let sock = UdpSocket::bind(&addr)?;

            Ok(Server(sock))
        }

        pub(super) fn build(
            self,
            receive: impl FnMut(Bytes) -> Result<Option<Message>, Error> + Send + Sync + 'static,
        ) -> impl Stream<Item = Message, Error = Error> {
            emit("Setting up for UDP");

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
            // All datagrams are considered a valid message
            let src = src.take().freeze();

            (self.0)(src)
        }
    }
}

mod tcp {
    use super::*;

    use futures::stream::{futures_unordered::FuturesUnordered, Fuse, StreamFuture};

    use tokio::{
        codec::{Decoder, FramedRead},
        net::tcp::TcpListener,
        timer::Timeout,
    };

    pub(super) struct Server(TcpListener);

    impl Server {
        pub(super) fn bind(addr: &SocketAddr) -> Result<Self, Error> {
            let listener = TcpListener::bind(&addr)?;

            Ok(Server(listener))
        }

        pub(super) fn build(
            self,
            keep_alive: Duration,
            receive: impl FnMut(Bytes) -> Result<Option<Message>, Error> + Send + Sync + 'static + Clone,
        ) -> impl Stream<Item = Message, Error = Error> {
            emit("Setting up for TCP");

            self.0
                .incoming()
                .map(move |conn| {
                    let decode = Decode::new(receive.clone());
                    let protocol = FramedRead::new(conn, decode);

                    TimeoutStream::new(protocol, keep_alive)
                })
                .listen(1024)
        }
    }

    struct Listen<S>
    where
        S: Stream,
        S::Item: Stream,
        <S::Item as Stream>::Error: From<S::Error>,
    {
        stream: Fuse<S>,
        connections: FuturesUnordered<StreamFuture<S::Item>>,
        max: usize,
    }

    impl<S> Stream for Listen<S>
    where
        S: Stream,
        S::Item: Stream,
        <S::Item as Stream>::Error: From<S::Error>,
    {
        type Item = <S::Item as Stream>::Item;
        type Error = <S::Item as Stream>::Error;

        fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
            'poll_conns: loop {
                // Fill up our accepted connections
                'fill_conns: while self.connections.len() < self.max {
                    let stream = match self.stream.poll()? {
                        Async::Ready(Some(s)) => s.into_future(),
                        Async::Ready(None) | Async::NotReady => break 'fill_conns,
                    };

                    self.connections.push(stream.into_future());
                }

                // Try polling the stream
                // NOTE: We're assuming the unordered list will
                // always make forward progress polling futures
                // even if one future is particularly chatty
                match self.connections.poll() {
                    // We have an item from a connection
                    Ok(Async::Ready(Some((Some(item), stream)))) => {
                        self.connections.push(stream.into_future());
                        return Ok(Async::Ready(Some(item)));
                    }
                    // A connection has closed
                    // Drop the stream and loop back
                    // This will mean attempting to accept a new connnection
                    Ok(Async::Ready(Some((None, _stream)))) => continue 'poll_conns,
                    // The queue is empty or nothing is ready
                    Ok(Async::Ready(None)) | Ok(Async::NotReady) => break 'poll_conns,
                    // An error occurred
                    // Drop the stream, but return the error
                    // FIXME: Some errors should be recoverable
                    Err((err, _stream)) => {
                        return Err(err);
                    }
                }
            }

            // If we've gotten this far, then there are no events for us to process
            // and nothing was ready, so figure out if we're not done yet  or if
            // we've reached the end.
            if self.stream.is_done() {
                Ok(Async::Ready(None))
            } else {
                Ok(Async::NotReady)
            }
        }
    }

    trait StreamListenExt: Stream {
        fn listen(self, max_connections: usize) -> Listen<Self>
        where
            Self: Sized,
            Self::Item: Stream,
            <Self::Item as Stream>::Error: From<Self::Error>,
        {
            Listen {
                stream: self.fuse(),
                connections: FuturesUnordered::new(),
                max: max_connections,
            }
        }
    }

    impl<S> StreamListenExt for S where S: Stream {}

    pub struct Decode<F> {
        next_index: usize,
        receive: F,
    }

    impl<F> Decode<F> {
        pub fn new(receive: F) -> Self {
            Decode {
                next_index: 0,
                receive,
            }
        }
    }

    impl<F> Decoder for Decode<F>
    where
        F: FnMut(Bytes) -> Result<Option<Message>, Error>,
    {
        type Item = Message;
        type Error = Error;

        fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            // Messages are separated by null bytes
            let sep_offset = src[self.next_index..].iter().position(|b| *b == b'\0');

            if let Some(offset) = sep_offset {
                let sep_index = offset + self.next_index;
                self.next_index = 0;
                let src = src.split_to(sep_index + 1).freeze();

                (self.receive)(src.slice_to(src.len() - 1))
            } else {
                self.next_index = src.len();

                Ok(None)
            }
        }

        fn decode_eof(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            Ok(match self.decode(src)? {
                Some(frame) => Some(frame),
                None => {
                    if src.is_empty() {
                        None
                    } else {
                        let src = src.take().freeze();
                        self.next_index = 0;

                        (self.receive)(src)?
                    }
                }
            })
        }
    }

    struct TimeoutStream<S> {
        stream: Timeout<S>,
    }

    impl<S> TimeoutStream<S>
    where
        S: Stream,
    {
        fn new(stream: S, keep_alive: Duration) -> Self {
            increment!(server.tcp_conn_accept);

            TimeoutStream {
                stream: Timeout::new(stream, keep_alive),
            }
        }
    }

    impl<S> Drop for TimeoutStream<S> {
        fn drop(&mut self) {
            increment!(server.tcp_conn_close);
        }
    }

    impl<S> Stream for TimeoutStream<S>
    where
        S: Stream,
    {
        type Item = S::Item;
        type Error = S::Error;

        fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
            match self.stream.poll() {
                Err(ref e) if e.is_elapsed() => {
                    increment!(server.tcp_conn_timeout);

                    Ok(Async::Ready(None))
                },
                Err(e) => match e.into_inner() {
                    Some(e) => Err(e),
                    None => Ok(Async::Ready(None)),
                },
                Ok(item) => Ok(item),
            }
        }
    }
}
