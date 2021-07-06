use crate::support::*;

pub fn test() {
    let mut server = server::builder().udp_max_chunks(3).udp();
    let mut sock = udp::sock();

    // Split a message into 5 chunks
    let msg_chunks = net_chunks!(5, {
        "host": "foo",
        "short_message": "this is a short message but long enough for 5 chunks"
    });

    assert_eq!(5, msg_chunks.len());

    for (i, chunk) in msg_chunks.into_iter().enumerate() {
        sock.send(net_chunks![
            ..udp_chunk(0, i as u8, 5, chunk)
        ]);
    }

    assert_eq!(0, server.received());

    sock.send(net_chunks![
        ..net_chunks!({
            "host": "foo",
            "short_message": "bar"
        })
    ]);

    server.receive(|received| {
        assert_eq!("bar", received["@m"]);
    });

    assert_eq!(1, server.received());

    server.close();
}
