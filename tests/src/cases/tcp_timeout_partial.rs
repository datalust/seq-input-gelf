use std::{
    thread,
    time::Duration,
};

use crate::support::*;

pub fn test() {
    let mut server = server::builder().tcp_keep_alive_secs(3).tcp();
    let mut stream = tcp::stream();

    thread::sleep(Duration::from_secs(1));

    stream.write(net_chunks![
        ..net_chunks!({
            "host": "foo",
            "short_message": "bar"
        })
    ]);

    thread::sleep(Duration::from_secs(3));

    stream.write(net_chunks![..tcp_delim()]);

    assert_eq!(0, server.received());

    stream.close();
    server.close();
}
