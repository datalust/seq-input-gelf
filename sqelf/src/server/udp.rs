use std::net::SocketAddr;

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
    Bytes,
    BytesMut,
};

use futures::{
    Stream,
    StreamExt,
};

use tokio::net::UdpSocket;

use tokio_util::{
    codec::Decoder,
    udp::UdpFramed,
};

pub(super) struct Server(UdpSocket);

impl Server {
    pub(super) async fn bind(addr: &SocketAddr) -> Result<Self, Error> {
        let sock = UdpSocket::bind(&addr).await?;

        Ok(Server(sock))
    }

    pub(super) fn build(
        self,
        receive: impl FnMut(Bytes) -> Result<Option<Message>, Error> + Unpin,
    ) -> impl Stream<Item = Result<Received, Error>> {
        emit("Setting up for UDP");

        UdpFramed::new(self.0, Decode(receive)).map(|r| r.map(|(msg, _)| msg))
    }
}

struct Decode<F>(F);

impl<F> Decoder for Decode<F>
where
    F: FnMut(Bytes) -> Result<Option<Message>, Error> + Unpin,
{
    type Item = Received;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // NOTE: We don't use `?` here because we never want to carry results
        // We always want to match them and deal with error cases directly

        // All datagrams are considered a valid message
        let src = src.split_to(src.len()).freeze();

        if src.is_empty() {
            // As per the contract of `Decoder`, we return `None`
            // here to indicate more data is needed to complete a frame
            return Ok(None);
        }

        Ok((self.0)(src).into_received())
    }
}
