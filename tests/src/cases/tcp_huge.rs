use std::str;

use crate::support::*;

pub fn test() {
    let mut server = server::builder().tcp_max_size_bytes(1024 * 32).tcp();
    let mut stream = tcp::stream();

    let short_message = str::from_utf8(&[b'a'; 1024 * 12]).unwrap();

    stream.write(net_chunks![
        ..net_chunks!({
            "host": "foo",
            "short_message": short_message
        }),
        ..tcp_delim()
    ]);

    server.receive(|received| {
        assert_eq!(short_message, received["@m"]);
    });

    assert_eq!(1, server.received());

    stream.close();
    server.close();
}
