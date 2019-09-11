use crate::support::*;

pub fn test() {
    let server = server::tcp();
    server.close();
}
