#[macro_use]
extern crate serde_derive;

pub mod io;
pub mod process;
pub mod receive;

use std::{error, net::SocketAddr};

use tokio::{
    net::udp::{UdpFramed, UdpSocket},
    prelude::*,
};

use futures::{future::lazy, sync::mpsc};

#[derive(Debug, Default)]
pub struct Config {
    pub receive: receive::Config,
    pub process: process::Config,
}

fn main() -> Result<(), Box<error::Error>> {
    let config = Config::default();

    eprintln!("{:#?}", config);

    let addr: SocketAddr = config.receive.bind.parse()?;
    let sock = UdpSocket::bind(&addr)?;

    let (tx, rx) = mpsc::channel(config.process.unprocessed_capacity);

    let server = lazy(move || {
        // Spawn a background task to process events
        let process = process::Process::new(config.process);
        tokio::spawn(lazy(move || {
            rx.for_each(move |msg| {
                let read_as_clef = |msg: receive::Message| {
                    process.read_as_clef(msg)?;

                    Ok(())
                };

                read_as_clef(msg).map_err(|e: receive::Error| eprintln!("{}", e))
            })
        }));

        // Accept and process incoming GELF messages over UDP
        UdpFramed::new(sock, receive::Gelf::new(config.receive))
            .for_each(move |(msg, _)| {
                let tx = tx.clone();

                tx.send(msg).map(|_| ()).map_err(Into::into)
            })
            .map_err(|e| eprintln!("{}", e))
    });

    tokio::run(server);

    Ok(())
}
