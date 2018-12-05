use std::{
    cmp,
    collections::{hash_map, BTreeMap, HashMap},
    error,
    io::{self, Read},
};

use bytes::{Buf, Bytes, BytesMut, IntoBuf};

use libflate::{gzip, zlib};

use tokio::codec::Decoder;

use crate::io::MemRead;

pub type Error = Box<error::Error + Send + Sync>;

#[derive(Debug)]
pub struct Config {
    pub bind: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bind: "0.0.0.0:12201".to_owned(),
        }
    }
}

/**
A decoder for GELF messages.

A message may be chunked and compressed.
This decoder won't attempt to validate that the contents
of the message itself conforms to the GELF specification.
*/
#[derive(Debug)]
pub struct Gelf {
    chunks: HashMap<u64, Chunks>,
    arrival: BTreeMap<u64, u64>,
}

impl Gelf {
    pub fn new(_: Config) -> Self {
        Gelf {
            chunks: HashMap::new(),
            arrival: BTreeMap::new(),
        }
    }
}

impl Decoder for Gelf {
    type Item = Message;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut src = src.take().freeze();

        let magic = Message::peek_magic_bytes(&src);

        if magic == Some(Message::MAGIC_CHUNKED) {
            let header = ChunkHeader::get(&mut src)?;

            // If the message is just a single chunk we can treat it
            // like an unchunked message.
            if header.seq_num == 0 && header.seq_count == 1 {
                let magic = Message::peek_magic_bytes(&src);

                return Ok(Message::single(magic.and_then(Compression::detect), src));
            }

            let chunk = Chunk {
                seq: header.seq_num,
                bytes: src,
            };

            match self.chunks.entry(header.id) {
                // Begin a new message with the given chunk
                hash_map::Entry::Vacant(entry) => {
                    entry.insert(Chunks::new(header.seq_count, chunk));

                    Ok(None)
                }
                // Add a chunk to an existing message
                // If the chunk completes the message then return it
                hash_map::Entry::Occupied(mut entry) => {
                    let chunks = entry.get_mut();

                    chunks.insert(chunk);
                    if chunks.is_complete() {
                        let (_, chunks) = entry.remove_entry();

                        Ok(Message::chunked(
                            chunks.inner.into_iter().map(|(_, chunk)| chunk),
                        ))
                    } else {
                        Ok(None)
                    }
                }
            }
        } else {
            // Return a message containing a single chunk
            Ok(Message::single(magic.and_then(Compression::detect), src))
        }
    }
}

#[derive(Debug)]
struct Chunks {
    expected_total: u8,
    inner: BTreeMap<u8, Bytes>,
}

struct Chunk {
    seq: u8,
    bytes: Bytes,
}

impl Chunks {
    fn new(expected_total: u8, chunk: Chunk) -> Self {
        let mut inner = BTreeMap::new();
        inner.insert(chunk.seq, chunk.bytes);

        Chunks {
            expected_total,
            inner,
        }
    }

    fn insert(&mut self, chunk: Chunk) {
        self.inner.insert(chunk.seq, chunk.bytes);
    }

    fn is_complete(&self) -> bool {
        self.expected_total as usize == self.inner.len()
    }
}

/**
A raw GELF message.
*/
#[derive(Debug, PartialEq, Eq)]
pub struct Message(MessageInner);

#[derive(Debug, PartialEq, Eq)]
enum MessageInner {
    /**
    A message consisting of a single chunk.

    The chunk may be compressed.
    */
    Single {
        compression: Option<Compression>,
        bytes: Bytes,
    },
    /**
    A message consisting of multiple chunks.

    The chunks may be compressed, but the compression
    isn't known until reading starts. Chunks are expected
    to have been compressed before being chunked, so that
    individual chunks aren't individually compressed.
    */
    Chunked { chunks: Vec<Bytes> },
}

struct ChunkHeader {
    id: u64,
    seq_num: u8,
    seq_count: u8,
}

impl ChunkHeader {
    const SIZE: usize = 12;

    fn get(buf: &mut Bytes) -> Result<Self, Error> {
        if buf.len() < Self::SIZE {
            panic!("too small")
        }

        let mut buf = buf.split_to(Self::SIZE).into_buf();

        let _magic = [buf.get_u8(), buf.get_u8()];

        let id = buf.get_u64_be();
        let seq_num = buf.get_u8();
        let seq_count = buf.get_u8();

        if seq_num >= seq_count {
            panic!("invalid sequence")
        }

        Ok(ChunkHeader {
            id,
            seq_num,
            seq_count,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compression {
    Gzip,
    Zlib,
}

impl Message {
    const MAGIC_CHUNKED: [u8; 2] = [0x1e, 0x0f];

    fn single(compression: Option<Compression>, src: Bytes) -> Option<Self> {
        if src.len() == 0 {
            return None;
        }

        debug_assert_eq!(
            Self::peek_magic_bytes(&src).and_then(Compression::detect),
            compression
        );

        Some(Message(MessageInner::Single {
            compression,
            bytes: src,
        }))
    }

    fn chunked(chunks: impl IntoIterator<Item = Bytes>) -> Option<Self> {
        let chunks: Vec<_> = chunks.into_iter().collect();

        if chunks.len() == 0 {
            return None;
        }

        Some(Message(MessageInner::Chunked { chunks }))
    }

    fn peek_magic_bytes(src: &[u8]) -> Option<[u8; 2]> {
        if src.len() < 2 {
            return None;
        }

        let mut header = [0; 2];
        header.copy_from_slice(&src[0..2]);

        Some(header)
    }

    fn compression(&self) -> Option<Compression> {
        match &self.0 {
            MessageInner::Single { compression, .. } => *compression,
            MessageInner::Chunked { chunks } => chunks
                .first()
                .and_then(|chunk| Self::peek_magic_bytes(&chunk))
                .and_then(Compression::detect),
        }
    }
}

impl MemRead for Message {
    type Reader = Reader;

    fn bytes(&self) -> Option<&[u8]> {
        match &self.0 {
            MessageInner::Single {
                bytes,
                compression: None,
            } => Some(&*bytes),
            _ => None,
        }
    }

    fn into_reader(self) -> io::Result<Reader> {
        let compression = self.compression();

        let body = ChunkRead {
            chunk: 0,
            cursor: 0,
            msg: self.0,
        };

        let reader = match compression {
            Some(Compression::Gzip) => Reader(ReaderInner::Gzip(gzip::Decoder::new(body)?)),
            Some(Compression::Zlib) => Reader(ReaderInner::Zlib(zlib::Decoder::new(body)?)),
            None => Reader(ReaderInner::Uncompressed(body)),
        };

        Ok(reader)
    }
}

/**
A reader for a message.
*/
pub struct Reader(ReaderInner);

enum ReaderInner {
    Uncompressed(ChunkRead),
    Gzip(gzip::Decoder<ChunkRead>),
    Zlib(zlib::Decoder<ChunkRead>),
}

impl Read for Reader {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        match &mut self.0 {
            ReaderInner::Uncompressed(msg) => msg.read(b),
            ReaderInner::Gzip(msg) => msg.read(b),
            ReaderInner::Zlib(msg) => msg.read(b),
        }
    }
}

struct ChunkRead {
    chunk: usize,
    cursor: usize,
    msg: MessageInner,
}

impl Read for ChunkRead {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        match &mut self.msg {
            MessageInner::Single { bytes, .. } => {
                if b.len() == 0 {
                    return Ok(0);
                }

                let readable = &bytes[self.cursor..];

                let read = cmp::min(readable.len(), b.len());
                b[0..read].copy_from_slice(&readable[0..read]);
                self.cursor += read;

                Ok(read)
            }
            MessageInner::Chunked { chunks, .. } => {
                let mut b = b;
                let mut total = 0;

                while b.len() > 0 {
                    if let Some(bytes) = chunks.get(self.chunk) {
                        let readable = &bytes[self.cursor..];

                        let read = cmp::min(readable.len(), b.len());
                        b[0..read].copy_from_slice(&readable[0..read]);

                        if read == readable.len() {
                            self.chunk += 1;
                            self.cursor = 0;
                        } else {
                            self.cursor += read;
                        }

                        total += read;
                        b = &mut b[read..];
                    } else {
                        break;
                    }
                }

                Ok(total)
            }
        }
    }
}

impl Compression {
    const MAGIC_GZIP: [u8; 2] = [0x1f, 0x8b];
    const MAGIC_ZLIB: u8 = 0x78;

    fn detect(header: [u8; 2]) -> Option<Compression> {
        match header {
            Self::MAGIC_GZIP => Some(Compression::Gzip),
            header
                if header[0] == Self::MAGIC_ZLIB
                    && ((u16::from(header[0]) << 8) + u16::from(header[1])) % 31 == 0 =>
            {
                Some(Compression::Zlib)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    use libflate::{gzip, zlib};

    use byteorder::{BigEndian, ByteOrder};

    fn chunk(id: u64, seq_num: u8, seq_total: u8, bytes: &[u8]) -> BytesMut {
        let mut header = vec![0x1e, 0x0f];

        let mut idb = [0; 8];

        BigEndian::write_u64(&mut idb, id);

        header.extend(&idb);
        header.push(seq_num);
        header.push(seq_total);
        header.extend(bytes);

        header.into()
    }

    fn zlib(bytes: &[u8]) -> BytesMut {
        let mut encoder = zlib::Encoder::new(Vec::new()).expect("failed to build zlib");

        encoder.write_all(bytes).expect("failed to encode bytes");

        encoder
            .finish()
            .into_result()
            .expect("failed to finish encoding")
            .into()
    }

    fn gzip(bytes: &[u8]) -> BytesMut {
        let mut encoder = gzip::Encoder::new(Vec::new()).expect("failed to build gzip");

        encoder.write_all(bytes).expect("failed to encode bytes");

        encoder
            .finish()
            .into_result()
            .expect("failed to finish encoding")
            .into()
    }

    #[test]
    fn message_empty() {
        let mut gelf = Gelf::new(Default::default());

        let msg = gelf
            .decode(&mut BytesMut::from(b"" as &[u8]))
            .expect("failed to decode message");

        assert!(msg.is_none());
    }

    #[test]
    fn message_unchunked() {
        let mut gelf = Gelf::new(Default::default());

        let msg = gelf
            .decode(&mut BytesMut::from(b"Hello!" as &[u8]))
            .expect("failed to decode message")
            .expect("missing message value");

        let expected = Message(MessageInner::Single {
            compression: None,
            bytes: Bytes::from(b"Hello!" as &[u8]),
        });

        assert_eq!(expected, msg);
    }

    #[test]
    fn read_message_unchunked_uncompressed() {
        let mut gelf = Gelf::new(Default::default());

        let mut msg = gelf
            .decode(&mut BytesMut::from(b"Hello!" as &[u8]))
            .expect("failed to decode message")
            .expect("missing message value")
            .into_reader()
            .expect("failed to build reader");

        let mut read = String::new();
        msg.read_to_string(&mut read)
            .expect("failed to read message");

        assert_eq!("Hello!", read);
    }

    #[test]
    fn read_message_unchunked_gzip() {
        let mut gelf = Gelf::new(Default::default());

        let mut msg = gelf
            .decode(&mut gzip(b"Hello!"))
            .expect("failed to decode message")
            .expect("missing message value")
            .into_reader()
            .expect("failed to build reader");

        let mut read = String::new();
        msg.read_to_string(&mut read)
            .expect("failed to read message");

        assert_eq!("Hello!", read);
    }

    #[test]
    fn read_message_unchunked_zlib() {
        let mut gelf = Gelf::new(Default::default());

        let mut msg = gelf
            .decode(&mut zlib(b"Hello!"))
            .expect("failed to decode message")
            .expect("missing message value")
            .into_reader()
            .expect("failed to build reader");

        let mut read = String::new();
        msg.read_to_string(&mut read)
            .expect("failed to read message");

        assert_eq!("Hello!", read);
    }

    #[test]
    fn message_single_chunk() {
        let mut gelf = Gelf::new(Default::default());

        let msg = gelf
            .decode(&mut chunk(0, 0, 1, b"Hello!"))
            .expect("failed to decode message")
            .expect("missing message value");

        let expected = Message(MessageInner::Single {
            compression: None,
            bytes: Bytes::from(b"Hello!" as &[u8]),
        });

        assert_eq!(expected, msg);
    }

    #[test]
    fn message_chunked_empty() {
        let mut gelf = Gelf::new(Default::default());

        let msg = gelf
            .decode(&mut chunk(0, 0, 1, b""))
            .expect("failed to decode message");

        assert!(msg.is_none());
    }

    #[test]
    fn message_multiple_chunks() {
        let mut gelf = Gelf::new(Default::default());

        let partial = gelf
            .decode(&mut chunk(0, 0, 3, b"Hello"))
            .expect("failed to decode message");

        assert!(partial.is_none());

        let partial = gelf
            .decode(&mut chunk(0, 2, 3, b"!"))
            .expect("failed to decode message");

        assert!(partial.is_none());

        let msg = gelf
            .decode(&mut chunk(0, 1, 3, b" World"))
            .expect("failed to decode message")
            .expect("missing message value");

        let expected = Message(MessageInner::Chunked {
            chunks: vec![
                Bytes::from(b"Hello" as &[u8]),
                Bytes::from(b" World" as &[u8]),
                Bytes::from(b"!" as &[u8]),
            ],
        });

        assert_eq!(expected, msg);
    }

    #[test]
    fn read_message_chunked_uncompressed() {
        let mut gelf = Gelf::new(Default::default());

        gelf.decode(&mut chunk(0, 0, 3, b"Hello"))
            .expect("failed to decode message");

        gelf.decode(&mut chunk(0, 2, 3, b"!"))
            .expect("failed to decode message");

        let mut msg = gelf
            .decode(&mut chunk(0, 1, 3, b" World"))
            .expect("failed to decode message")
            .expect("missing message value")
            .into_reader()
            .expect("failed to build reader");

        let mut read = String::new();
        msg.read_to_string(&mut read)
            .expect("failed to read message");

        assert_eq!("Hello World!", read);
    }

    #[test]
    fn read_message_chunked_zlib() {
        let mut buf = zlib(b"Hello World!");

        let (mut chunk_1, mut chunk_2, mut chunk_3) = (&buf[0..2], &buf[2..4], &buf[4..]);

        let mut gelf = Gelf::new(Default::default());

        gelf.decode(&mut chunk(0, 0, 3, chunk_1))
            .expect("failed to decode message");

        gelf.decode(&mut chunk(0, 2, 3, chunk_3))
            .expect("failed to decode message");

        let mut msg = gelf
            .decode(&mut chunk(0, 1, 3, chunk_2))
            .expect("failed to decode message")
            .expect("missing message value")
            .into_reader()
            .expect("failed to build reader");

        let mut read = String::new();
        msg.read_to_string(&mut read)
            .expect("failed to read message");

        assert_eq!("Hello World!", read);
    }

    #[test]
    fn read_message_chunked_gzip() {
        let mut buf = gzip(b"Hello World!");

        let (mut chunk_1, mut chunk_2, mut chunk_3) = (&buf[0..2], &buf[2..4], &buf[4..]);

        let mut gelf = Gelf::new(Default::default());

        gelf.decode(&mut chunk(0, 0, 3, chunk_1))
            .expect("failed to decode message");

        gelf.decode(&mut chunk(0, 2, 3, chunk_3))
            .expect("failed to decode message");

        let mut msg = gelf
            .decode(&mut chunk(0, 1, 3, chunk_2))
            .expect("failed to decode message")
            .expect("missing message value")
            .into_reader()
            .expect("failed to build reader");

        let mut read = String::new();
        msg.read_to_string(&mut read)
            .expect("failed to read message");

        assert_eq!("Hello World!", read);
    }
}
