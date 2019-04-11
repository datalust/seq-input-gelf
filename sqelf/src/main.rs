#[macro_use]
extern crate serde_derive;

#[macro_use]
pub mod error;

mod diagnostics;
pub mod io;
pub mod process;
pub mod receive;
pub mod server;

mod config;

pub use self::config::Config;
use self::{
    diagnostics::emit_err,
    error::{
        Error,
        err_msg,
    },
};

fn main() {
    if let Err(err) = run() {
        emit_err(&err, "GELF input failed");
        std::process::exit(1);
    }
}

fn run() -> Result<(), error::StdError> {
    let config = Config::from_env()?;

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
    match tokio::runtime::current_thread::block_on_all(server) {
        Ok(()) | Err(server::Exit::Clean) => Ok(()),
        _ => Err(err_msg("Server execution failed").into())
    }
}
