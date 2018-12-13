use std::net::SocketAddr;

use tokio::{
    codec::Decoder,
    net::udp::{UdpFramed, UdpSocket},
    prelude::*,
};

use futures::{future::lazy, sync::mpsc};

pub type Error = failure::Error;

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
    receive: impl Decoder<Item = Message, Error = Error> + Send + Sync + 'static,
    handle: impl Fn(Message) -> Result<(), Error> + Send + Sync + 'static,
) -> Result<impl Future<Item = (), Error = ()>, Error> {
    let addr: SocketAddr = config.bind.parse()?;
    let sock = UdpSocket::bind(&addr)?;

    let (tx, rx) = mpsc::channel(config.unprocessed_capacity);

    Ok(lazy(move || {
        // Spawn a background task to process events
        tokio::spawn(lazy(move || {
            rx.for_each(move |msg| {
                handle(msg).or_else(|e: Error| {
                    eprintln!("processing failed: {}", e);

                    Ok(())
                })
            })
        }));

        // Accept and process incoming GELF messages over UDP
        UdpFramed::new(sock, receive)
            .for_each(move |(msg, _)| {
                let tx = tx.clone();

                tx.send(msg).map(|_| ()).or_else(|e| {
                    eprintln!("sending failed: {}", e);

                    Ok(())
                })
            })
            .or_else(|e| {
                eprintln!("receiving failed: {}", e);

                Ok(())
            })
    }))
}
