use std::io::{Bytes, Error, ErrorKind, Read, Result, Write};

pub struct Bitstream<T> {
    inner: Bytes<T>,
    next_bits: u128,
    next_bits_length: usize,
}

impl<T: Read> Bitstream<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: inner.bytes(),
            next_bits: 0,
            next_bits_length: 0,
        }
    }

    pub fn next_bits(&mut self, n: usize) -> Result<u64> {
        while self.next_bits_length < n {
            let b = match self.inner.next().transpose()? {
                Some(b) => b as u128,
                None => {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        "unexpected end of bitstream",
                    ))
                }
            };
            self.next_bits = (self.next_bits << 8) | b;
            self.next_bits_length += 8;
        }
        Ok(
            ((self.next_bits >> (self.next_bits_length - n)) & (0xffff_ffff_ffff_ffff >> (64 - n)))
                as u64,
        )
    }

    pub fn read_bits(&mut self, n: usize) -> Result<u64> {
        let ret = self.next_bits(n)?;
        self.next_bits_length -= n;
        Ok(ret)
    }
}

pub struct BitstreamWriter<T: Write> {
    inner: T,
    next_bits: u128,
    next_bits_length: usize,
}

impl<T: Write> BitstreamWriter<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            next_bits: 0,
            next_bits_length: 0,
        }
    }

    // Writes the given bits to the bitstream. If an error occurs, it is undefined how many bits
    // were actually written to the underlying bitstream.
    pub fn write_bits(&mut self, bits: u64, mut len: usize) -> Result<()> {
        while len >= 128 {
            self.write_bits(0, 64)?;
            len -= 64;
        }
        if len > 64 {
            self.write_bits(0, len - 64)?;
            len = 64;
        }
        self.next_bits = (self.next_bits << len) | bits as u128;
        self.next_bits_length += len;
        while self.next_bits_length >= 8 {
            let next_byte = (self.next_bits >> (self.next_bits_length - 8)) as u8;
            self.inner.write_all(&[next_byte])?;
            self.next_bits_length -= 8;
        }
        Ok(())
    }

    // Writes the remaining bits to the underlying writer if there are any, and flushes it. If the
    // bitstream is not byte-aligned, zero-bits will be appended until it is.
    pub fn flush(&mut self) -> Result<()> {
        if self.next_bits_length > 0 {
            let next_byte = (self.next_bits << (8 - self.next_bits_length)) as u8;
            self.inner.write_all(&[next_byte])?;
            self.next_bits_length = 0;
        }
        self.inner.flush()
    }
}

impl<T: Write> Drop for BitstreamWriter<T> {
    fn drop(&mut self) {
        // if users need the error, they should explicitly invoke flush before dropping
        let _ = self.flush();
    }
}
