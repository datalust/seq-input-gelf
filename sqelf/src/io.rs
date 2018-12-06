use std::io;

pub trait MemRead {
    type Reader: io::Read;

    fn bytes(&self) -> Option<&[u8]>;
    fn into_reader(self) -> io::Result<Self::Reader>;
}

impl<'a> MemRead for &'a [u8] {
    type Reader = io::Cursor<&'a [u8]>;

    fn bytes(&self) -> Option<&[u8]> {
        Some(&self)
    }

    fn into_reader(self) -> io::Result<Self::Reader> {
        Ok(io::Cursor::new(self))
    }
}
