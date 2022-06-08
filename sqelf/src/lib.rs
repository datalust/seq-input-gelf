/*!
A lightweight GELF server that writes CLEF to stdout.

The server is split into a few main components, in order of where they appear in the processing of a log event:

- **Server**: An asynchronous UDP/TCP server built on `tokio` that handles the network.
- **Receive**: Assembles complete GELF messages from their chunked, compressed, out-of-order
blocks arriving from the network.
- **Process**: Deserializes GELF messages and maps them into CLEF. This is where any transformations
over properties are made.
*/

#![recursion_limit = "256"]
#![deny(unsafe_code)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate serde_derive;

#[macro_use]
pub mod diagnostics;

#[macro_use]
extern crate anyhow;

pub mod config;
pub mod io;
pub mod process;
pub mod receive;
pub mod server;

pub use self::{anyhow::Error, config::Config};
