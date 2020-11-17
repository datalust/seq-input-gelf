use std::{
    cmp,
    io,
    net::SocketAddr,
    pin::Pin,
    time::Duration,
};

use crate::{
    diagnostics::*,
    receive::Message,
    server::{
        OptionMessageExt,
        Received,
    },
};

use anyhow::Error;

use bytes::{
    Buf,
    Bytes,
    BytesMut,
};

use futures::{
    future::{
        self,
        Future,
    },
    stream::{
        futures_unordered::FuturesUnordered,
        Fuse,
        Stream,
        StreamExt,
        StreamFuture,
    },
    task::{
        Context,
        Poll,
    },
};

use pin_utils::unsafe_pinned;

use tokio::{
    net::{
        TcpListener,
        TcpStream,
    },
    time::{
        timeout,
        Timeout,
    },
};

use tokio_util::codec::{
    Decoder,
    FramedRead,
};

pub(super) struct Server(TcpIncoming);

impl Server {
    pub(super) async fn bind(addr: &SocketAddr) -> Result<Self, Error> {
        let listener = TcpListener::bind(&addr).await?;

        Ok(Server(TcpIncoming(listener)))
    }

    pub(super) fn build(
        self,
        keep_alive: Duration,
        max_size_bytes: usize,
        receive: impl FnMut(Bytes) -> Result<Option<Message>, Error>
            + Send
            + Sync
            + Unpin
            + Clone
            + 'static,
    ) -> impl Stream<Item = Result<Received, Error>> {
        emit("Setting up for TCP");

        self.0
            .filter_map(move |conn| {
                match conn {
                    // The connection was successfully established
                    // Create a new protocol reader over it
                    // It'll get added to the connection pool
                    Ok(conn) => {
                        let decode = Decode::new(max_size_bytes, receive.clone());
                        let protocol = FramedRead::new(conn, decode);

                        // NOTE: The timeout stream wraps _the protocol_
                        // That means it'll close the connection if it doesn't
                        // produce a valid message within the timeframe, not just
                        // whether or not it writes to the stream
                        future::ready(Some(TimeoutStream::new(protocol, keep_alive)))
                    }
                    // The connection could not be established
                    // Just ignore it
                    Err(_) => future::ready(None),
                }
            })
            .listen(1024)
    }
}

struct TcpIncoming(TcpListener);

impl Stream for TcpIncoming {
    type Item = io::Result<TcpStream>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        match self.0.poll_accept(cx) {
            Poll::Ready(Ok((conn, _))) => Poll::Ready(Some(Ok(conn))),
            Poll::Ready(Err(err)) => Poll::Ready(Some(Err(err))),
            Poll::Pending => Poll::Pending,
        }
    }
}

struct Listen<S>
where
    S: Stream,
    S::Item: Stream,
{
    accept: Fuse<S>,
    connections: FuturesUnordered<StreamFuture<S::Item>>,
    max: usize,
}

impl<S> Listen<S>
where
    S: Stream,
    S::Item: Stream,
{
    unsafe_pinned!(accept: Fuse<S>);
    unsafe_pinned!(connections: FuturesUnordered<StreamFuture<S::Item>>);
}

impl<S, T> Stream for Listen<S>
where
    S: Stream + Unpin,
    S::Item: Stream<Item = Result<T, Error>> + Unpin,
{
    type Item = Result<T, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        'poll_conns: loop {
            // Fill up our accepted connections
            'fill_conns: while self.connections.len() < self.max {
                let conn = match self.as_mut().accept().poll_next(cx) {
                    Poll::Ready(Some(s)) => s.into_future(),
                    Poll::Ready(None) | Poll::Pending => break 'fill_conns,
                };

                self.connections.push(conn);
            }

            // Try polling the stream
            // NOTE: We're assuming the unordered list will
            // always make forward progress polling futures
            // even if one future is particularly chatty
            match self.as_mut().connections().poll_next(cx) {
                // We have an item from a connection
                Poll::Ready(Some((Some(item), conn))) => {
                    match item {
                        // A valid item was produced
                        // Return it and put the connection back in the pool.
                        Ok(item) => {
                            self.connections.push(conn.into_future());

                            return Poll::Ready(Some(Ok(item)));
                        }
                        // An error occurred, probably IO-related
                        // In this case the connection isn't returned to the pool.
                        // It's closed on drop and the error is returned.
                        Err(err) => {
                            return Poll::Ready(Some(Err(err.into())));
                        }
                    }
                }
                // A connection has closed
                // Drop the connection and loop back
                // This will mean attempting to accept a new connection
                Poll::Ready(Some((None, _conn))) => continue 'poll_conns,
                // The queue is empty or nothing is ready
                Poll::Ready(None) | Poll::Pending => break 'poll_conns,
            }
        }

        // If we've gotten this far, then there are no events for us to process
        // and nothing was ready, so figure out if we're not done yet  or if
        // we've reached the end.
        if self.accept.is_done() {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}

trait StreamListenExt: Stream {
    fn listen(self, max_connections: usize) -> Listen<Self>
    where
        Self: Sized + Unpin,
        Self::Item: Stream + Unpin,
    {
        Listen {
            accept: self.fuse(),
            connections: FuturesUnordered::new(),
            max: max_connections,
        }
    }
}

impl<S> StreamListenExt for S where S: Stream {}

struct Decode<F> {
    max_size_bytes: usize,
    read_head: usize,
    discarding: bool,
    receive: F,
}

impl<F> Decode<F> {
    pub fn new(max_size_bytes: usize, receive: F) -> Self {
        Decode {
            read_head: 0,
            discarding: false,
            max_size_bytes,
            receive,
        }
    }
}

impl<F> Decoder for Decode<F>
where
    F: FnMut(Bytes) -> Result<Option<Message>, Error>,
{
    type Item = Received;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        'read_frame: loop {
            let read_to = cmp::min(self.max_size_bytes.saturating_add(1), src.len());

            // Messages are separated by null bytes
            let sep_offset = src[self.read_head..].iter().position(|b| *b == b'\0');

            match (self.discarding, sep_offset) {
                // A delimiter was found
                // Split it from the buffer and return
                (false, Some(offset)) => {
                    let frame_end = offset + self.read_head;

                    // The message is technically sitting right there
                    // for us, but since it's bigger than our max capacity
                    // we still discard it
                    if frame_end > self.max_size_bytes {
                        increment!(server.tcp_msg_overflow);

                        self.discarding = true;

                        continue 'read_frame;
                    }

                    self.read_head = 0;
                    let src = src.split_to(frame_end + 1).freeze();

                    return Ok((self.receive)(src.slice(..src.len() - 1))?.into_received());
                }
                // A delimiter wasn't found, but the incomplete
                // message is too big. Start discarding the input
                (false, None) if src.len() > self.max_size_bytes => {
                    increment!(server.tcp_msg_overflow);

                    self.discarding = true;

                    continue 'read_frame;
                }
                // A delimiter wasn't found
                // Move the read head forward so we'll check
                // from that position next time data arrives
                (false, None) => {
                    self.read_head = read_to;

                    // As per the contract of `Decoder`, we return `None`
                    // here to indicate more data is needed to complete a frame
                    return Ok(None);
                }
                // We're discarding input and have reached the end of the message
                // Advance the source buffer to the end of that message and try again
                (true, Some(offset)) => {
                    src.advance(offset + self.read_head + 1);
                    self.discarding = false;
                    self.read_head = 0;

                    continue 'read_frame;
                }
                // We're discarding input but haven't reached the end of the message yet
                (true, None) => {
                    src.advance(read_to);
                    self.read_head = 0;

                    if src.is_empty() {
                        // We still return `Ok` here, even though we have no intention
                        // of processing those bytes. Our maximum buffer size should still
                        // be limited by the initial capacity, since we're responsible for
                        // reserving additional capacity and aren't doing that
                        return Ok(None);
                    }

                    continue 'read_frame;
                }
            }
        }
    }

    fn decode_eof(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Ok(match self.decode(src)? {
            Some(frame) => Some(frame),
            None => {
                if src.is_empty() {
                    None
                } else {
                    let src = src.split_to(src.len()).freeze();
                    self.read_head = 0;

                    (self.receive)(src)?.into_received()
                }
            }
        })
    }
}

struct TimeoutStream<S> {
    keep_alive: Duration,
    stream: Timeout<StreamFuture<S>>,
}

impl<S> TimeoutStream<S>
where
    S: Stream + Unpin,
{
    fn new(stream: S, keep_alive: Duration) -> Self {
        increment!(server.tcp_conn_accept);

        TimeoutStream {
            keep_alive,
            stream: timeout(keep_alive, stream.into_future()),
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
    S: Stream + Unpin,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let unpinned = Pin::into_inner(self);

        match Pin::new(&mut unpinned.stream).poll(cx) {
            // The timeout has elapsed
            Poll::Ready(Err(_)) => {
                increment!(server.tcp_conn_timeout);

                Poll::Ready(None)
            }
            // The stream has produced an item
            // The timeout is reset
            Poll::Ready(Ok((item, stream))) => {
                unpinned.stream = timeout(unpinned.keep_alive, stream.into_future());

                Poll::Ready(item)
            }
            // The timeout hasn't elapsed and the stream hasn't produced an item
            Poll::Pending => Poll::Pending,
        }
    }
}
