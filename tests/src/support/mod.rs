use byteorder::{
    BigEndian,
    ByteOrder,
};

const SERVER_BIND: &'static str = "0.0.0.0:12202";
const SERVER_ADDR: &'static str = "127.0.0.1:12202";

pub mod server;
pub mod tcp;
pub mod udp;

pub use serde_json::Value;

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
    }};
    ($n:expr, {$($json:tt)*}) => {{
        let v = serde_json::to_vec(&json!({$($json)*})).unwrap();
        let size = v.len() / $n;

        v.chunks(size).map(Vec::from).collect::<Vec<Vec<_>>>()
    }}
}

pub(crate) fn udp_chunk(
    id: u64,
    seq_num: u8,
    seq_total: u8,
    bytes: impl AsRef<[u8]>,
) -> Vec<Vec<u8>> {
    let mut header = vec![0x1e, 0x0f];

    let mut idb = [0; 8];

    BigEndian::write_u64(&mut idb, id);

    header.extend(&idb);
    header.push(seq_num);
    header.push(seq_total);
    header.extend(bytes.as_ref());

    vec![header]
}

pub(crate) fn bytes(b: impl AsRef<[u8]>) -> Vec<Vec<u8>> {
    vec![b.as_ref().to_vec()]
}

pub(crate) fn tcp_delim() -> Vec<Vec<u8>> {
    vec![vec![b'\0']]
}

pub(crate) fn test_child(name: &str) -> bool {
    use std::{
        env,
        process::{
            Command,
            Stdio,
        },
    };

    let self_bin = env::args().next().expect("missing self command");

    let mut test = Command::new(self_bin)
        .arg(name)
        .stdout(Stdio::inherit())
        .spawn()
        .expect("failed to start child process");

    test.wait().expect("test execution failed").success()
}

macro_rules! cases {
    ($($case:ident),+) => {
        $(
            mod $case;
        )+

        pub(crate) fn test_all() {
            use std::process;

            let mut failed = Vec::new();

            $(
                if !$crate::support::test_child(stringify!($case)) {
                    failed.push(stringify!($case));
                }
            )+

            if failed.len() > 0 {
                eprintln!("test execution failed. Failures: {:#?}", failed);
                process::exit(1);
            }
        }

        pub(crate) fn test(name: impl AsRef<str>) {
            let name = name.as_ref();

            $(
                if name == stringify!($case) {
                    use sqelf::diagnostics;

                    diagnostics::init(diagnostics::Config {
                        min_level: diagnostics::Level::Debug,
                        ..Default::default()
                    });

                    println!("running {}...", stringify!($case));
                    self::$case::test();

                    diagnostics::stop().expect("failed to stop diagnostics");
                }
            )+
        }
    }
}
