use crate::support::*;

pub fn test() {
    let server = server::udp();
    server.close();
}
