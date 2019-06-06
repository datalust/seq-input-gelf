use std::{
    io::Write,
    net::{self, TcpStream, UdpSocket},
    thread,
    time::Duration,
};

use sqelf::{process, receive, server};

pub use serde_json::Value;

pub struct ToReceive {
    pub count: usize,
    pub when_sending: Vec<Vec<u8>>,
}

pub fn udp_expect(to_receive: ToReceive, check: impl Fn(&[Value])) {
    expect(server::Protocol::Udp, to_receive, check)
}

pub fn tcp_expect(to_receive: ToReceive, check: impl Fn(&[Value])) {
    expect(server::Protocol::Tcp, to_receive, check)
}

fn expect(protocol: server::Protocol, to_receive: ToReceive, check: impl Fn(&[Value])) {
    let ToReceive {
        count,
        when_sending,
    } = to_receive;

    assert!(
        when_sending.len() >= count,
        "cannot receive the expected number of messages based on the datagrams to send"
    );

    let (tx, rx) = crossbeam_channel::unbounded();

    // Build a server
    let mut server = server::build(
        server::Config {
            bind: server::Bind {
                addr: "0.0.0.0:12202".into(),
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

            move |msg| {
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

    // Send our datagrams
    match protocol {
        server::Protocol::Udp => {
            let sock = UdpSocket::bind("127.0.0.1:0").expect("failed to bind client socket");
            for dgram in when_sending {
                sock.send_to(&dgram, "127.0.0.1:12202")
                    .expect("failed to send datagram");
            }
        }
        server::Protocol::Tcp => {
            let mut stream =
                TcpStream::connect("127.0.0.1:12202").expect("failed to bind client stream");

            for chunk in when_sending {
                stream.write(&chunk).expect("failed to send chunk");
            }

            stream
                .shutdown(net::Shutdown::Both)
                .expect("failed to close connection");
        }
    }

    // Wait for the messages to be processed
    let mut received = Vec::with_capacity(count);
    while received.len() < count {
        let msg = rx
            .recv_timeout(Duration::from_secs(3))
            .expect("failed to receive a message");
        received.push(msg);
    }

    // Close the server
    handle.close();
    server.join().expect("failed to run server");

    // Check the messages received
    check(&received);
}

pub(crate) fn test_child(name: &str) {
    use std::{
        env,
        process::{Command, Stdio},
    };

    let self_bin = env::args().next().expect("missing self command");

    let mut test = Command::new(self_bin)
        .arg(name)
        .stdout(Stdio::inherit())
        .spawn()
        .expect("failed to start child process");

    test.wait().expect("test execution failed");
}

macro_rules! net_chunks {
    ($(..$net_chunks:expr),+) => {{
        let mut v = Vec::new();

        $(
            v.extend($net_chunks);
        )+

        v
    }};
    ({$($json:tt)*}) => {{
        let v = serde_json::to_vec(&json!({$($json)*})).unwrap();
        vec![v]
    }}
}

pub(crate) fn tcp_delim() -> Vec<Vec<u8>> {
    vec![vec![b'\0']]
}

macro_rules! cases {
    ($($case:ident),+) => {
        $(
            mod $case;
        )+

        pub(crate) fn test_all() {
            $(
                $crate::support::test_child(stringify!($case));
            )+
        }

        pub(crate) fn test(name: impl AsRef<str>) {
            let name = name.as_ref();

            $(
                if name == stringify!($case) {
                    println!("running {}...", stringify!($case));
                    self::$case::test();
                }
            )+
        }
    }
}
