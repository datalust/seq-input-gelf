#[macro_use]
extern crate serde_derive;

#[macro_use]
mod diagnostics;

#[macro_use]
pub mod error;

pub mod io;
pub mod process;
pub mod receive;
pub mod server;

mod config;

pub use self::config::Config;
use self::{
    diagnostics::{emit, emit_err},
    error::{err_msg, Error},
};

use std::panic::catch_unwind;

fn run() -> Result<(), error::StdError> {
    let config = Config::from_env()?;

    // Initialize diagnostics
    let mut diagnostics = diagnostics::init(config.diagnostics);

    emit("Starting GELF server");

    // The receiver for GELF messages
    let receive = {
        let mut receive = receive::build(config.receive);
        move |src| receive.decode(src)
    };

    // The processor for converting GELF into CLEF
    let process = {
        let process = process::build(config.process);
        move |msg| process.read_as_clef(msg)
    };

    // The server that drives the receiver and processor
    let server = server::build(config.server, receive, process)?;

    // Run the server and wait for it to exit
    let run_server = match tokio::runtime::current_thread::block_on_all(server) {
        Ok(()) | Err(server::Exit::Clean) => Ok(()),
        _ => Err(err_msg("Server execution failed").into()),
    };

    // Stop diagnostics
    let stop_diagnostics = diagnostics.stop_metrics().map_err(Into::into);

    run_server.and(stop_diagnostics)
}

fn main() {
    let run_server: Result<(), error::StdError> = catch_unwind(|| run())
        .map_err(|panic| error::unwrap_panic(panic).into())
        .and_then(|inner| inner);

    if let Err(err) = run_server {
        emit_err(&err, "GELF input failed");
        std::process::exit(1);
    }

    emit("GELF input stopped");
}
