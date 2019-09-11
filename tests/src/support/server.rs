use std::{
    sync::{
        Arc,
        Mutex,
    },
    thread,
    time::Duration,
};

use crossbeam_channel::{
    self,
    Receiver,
};

use serde_json::Value;

use sqelf::{
    process,
    receive,
    server,
};

use super::SERVER_BIND;

pub struct Server {
    server: thread::JoinHandle<()>,
    handle: server::Handle,
    received: Arc<Mutex<usize>>,
    rx: Receiver<Value>,
}

pub fn udp() -> Server {
    Server::new(server::Protocol::Udp)
}

pub fn tcp() -> Server {
    Server::new(server::Protocol::Tcp)
}

impl Server {
    fn new(protocol: server::Protocol) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let received = Arc::new(Mutex::new(0));

        let mut server = server::build(
            server::Config {
                bind: server::Bind {
                    addr: SERVER_BIND.into(),
                    protocol,
                },
                ..Default::default()
            },
            {
                let mut receive = receive::build(receive::Config {
                    ..Default::default()
                });

                move |src| receive.decode(src)
            },
            {
                let process = process::build(process::Config {
                    ..Default::default()
                });

                let received = received.clone();
                move |msg| {
                    *(received.lock().expect("poisoned lock")) += 1;

                    process.with_clef(msg, |clef| {
                        let json = serde_json::to_value(clef)?;
                        tx.send(json)?;

                        Ok(())
                    })
                }
            },
        )
        .expect("failed to build server");

        let handle = server.take_handle().expect("no server handle");
        let server = thread::spawn(move || server.run().expect("failed to run server"));

        // Wait for the server to become available
        thread::sleep(Duration::from_secs(1));

        Server {
            handle,
            server,
            rx,
            received,
        }
    }

    pub fn received(&mut self) -> usize {
        *(self.received.lock().expect("poisoned lock"))
    }

    pub fn receive(&mut self, f: impl FnOnce(Value)) {
        let msg = self
            .rx
            .recv_timeout(Duration::from_secs(3))
            .expect("failed to receive a message");

        f(msg)
    }

    pub fn close(self) {
        self.handle.close();
        self.server.join().expect("failed to run server");
    }
}
