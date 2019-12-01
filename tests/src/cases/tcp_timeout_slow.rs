use std::{
    thread,
    time::Duration,
};

use crate::support::*;

pub fn test() {
    let mut server = server::builder().tcp_keep_alive_secs(3).tcp();
    let mut stream = tcp::stream();

    for _ in 0..5 {
        stream.write(net_chunks![
            ..net_chunks!({
                "host": "foo",
                "short_message": "bar"
            }),
            ..tcp_delim()
        ]);

        server.receive(|_| { });
    
        thread::sleep(Duration::from_secs(1));
    }

    assert_eq!(5, server.received());

    stream.close();
    server.close();
}
