use crate::support::*;

pub fn test() {
    let mut server = server::udp();
    let mut sock = udp::sock();

    // Split a message into 2 chunks
    let msg_chunks = net_chunks!(2, {
        "host": "foo",
        "short_message": "bar"
    });

    assert_eq!(2, msg_chunks.len());

    sock.send(net_chunks![
        ..udp_chunk(0, 0, 2, &msg_chunks[0])
    ]);

    assert_eq!(0, server.received());

    sock.send(net_chunks![
        ..udp_chunk(0, 1, 2, &msg_chunks[1])
    ]);

    server.receive(|received| {
        assert_eq!("bar", received["@m"]);
    });

    server.close();
}
