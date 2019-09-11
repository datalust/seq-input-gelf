use std::net::UdpSocket;

use super::SERVER_ADDR;

pub struct Sock {
    inner: UdpSocket,
}

pub fn sock() -> Sock {
    Sock::new()
}

impl Sock {
    fn new() -> Sock {
        let sock = UdpSocket::bind("127.0.0.1:0").expect("failed to bind client socket");

        Sock { inner: sock }
    }

    pub fn send(&mut self, dgrams: Vec<Vec<u8>>) {
        for dgram in dgrams {
            self.inner
                .send_to(&dgram, SERVER_ADDR)
                .expect("failed to send datagram");
        }
    }
}
