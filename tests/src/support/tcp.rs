use std::{
    io::Write,
    net::{
        self,
        TcpStream,
    },
};

use super::SERVER_ADDR;

pub struct Stream {
    inner: TcpStream,
}

pub fn stream() -> Stream {
    Stream::new()
}

impl Stream {
    fn new() -> Self {
        let stream = TcpStream::connect(SERVER_ADDR).expect("failed to bind client stream");

        Stream { inner: stream }
    }

    pub fn write(&mut self, chunks: Vec<Vec<u8>>) {
        for chunk in chunks {
            self.inner.write(&chunk).expect("failed to send chunk");
        }
    }

    pub fn close(self) {
        let _ = self.inner.shutdown(net::Shutdown::Both);
    }
}
