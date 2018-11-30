use std::{
    cmp,
    collections::{BTreeMap, HashMap},
    error,
    io::{self, Read},
};

use bytes::{Bytes, BytesMut};

use libflate::{gzip, zlib};

use tokio::codec::Decoder;

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

#[derive(Debug)]
struct Chunks {
    expected_total: u8,
    inner: Vec<Chunk>,
}

#[derive(Debug)]
struct Chunk {
    bytes: Bytes,
}

impl Gelf {
    pub fn new(config: Config) -> Self {
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
        let mut header = [0; 2];
        header.copy_from_slice(&src[0..2]);

        if header == Message::MAGIC_CHUNKED {
            let bytes = src.take().freeze();
            unimplemented!("begin chunked payload");
        }

        let msg = Message(MessageInner::Single {
            compression: Compression::detect(header),
            bytes: src.take().freeze(),
        });

        Ok(Some(msg))
    }
}

/**
A raw GELF message.
*/
#[derive(Debug)]
pub struct Message(MessageInner);

#[derive(Debug)]
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

#[derive(Debug, Clone, Copy)]
enum Compression {
    Gzip,
    Zlib,
}

impl Message {
    const MAGIC_CHUNKED: [u8; 2] = [0x1e, 0x0f];

    fn compression(&self) -> Option<Compression> {
        match &self.0 {
            MessageInner::Single { compression, .. } => *compression,
            MessageInner::Chunked { chunks } => {
                unimplemented!("detect compression from first chunk")
            }
        }
    }

    /**
    Get a reader over the message bytes.

    The bytes will be dechunked and decompressed.
    */
    pub fn into_reader(self) -> Result<Reader, Error> {
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
                let readable = &bytes[self.cursor..];

                let read = cmp::min(readable.len(), b.len());
                b[0..read].copy_from_slice(&readable[0..read]);
                self.cursor += read;

                Ok(read)
            }
            MessageInner::Chunked { chunks, .. } => {
                unimplemented!("read chunked payload");
            }
        }
    }
}

impl Compression {
    const MAGIC_GZIP: [u8; 2] = [0x1f, 0x8b];
    const MAGIZ_ZLIB: u8 = 0x78;

    fn detect(header: [u8; 2]) -> Option<Compression> {
        match header {
            Self::MAGIC_GZIP => Some(Compression::Gzip),
            header
                if header[0] == Self::MAGIZ_ZLIB
                    && ((u16::from(header[0]) << 8) + u16::from(header[1])) % 31 == 0 =>
            {
                Some(Compression::Zlib)
            }
            _ => None,
        }
    }
}
