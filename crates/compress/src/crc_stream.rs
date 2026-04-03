use std::io::{Read, Write};

use crate::crc::Crc32;

pub struct CrcCalculatorStream<T> {
    inner: T,
    crc: Crc32,
    length_limit: Option<i64>,
}

impl<T> CrcCalculatorStream<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            crc: Crc32::new(),
            length_limit: None,
        }
    }

    pub fn with_length(inner: T, length: i64) -> Self {
        Self {
            inner,
            crc: Crc32::new(),
            length_limit: Some(length),
        }
    }

    pub fn total_bytes_slurped(&self) -> i64 {
        self.crc.total_bytes_read()
    }

    pub fn crc_i32(&self) -> i32 {
        self.crc.crc32_result_i32()
    }

    pub fn crc_u32(&self) -> u32 {
        self.crc.crc32_result_u32()
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T: Read> Read for CrcCalculatorStream<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(limit) = self.length_limit {
            if self.crc.total_bytes_read() >= limit {
                return Ok(0);
            }
            let remaining = (limit - self.crc.total_bytes_read()).max(0) as usize;
            let to_read = remaining.min(buf.len());
            if to_read == 0 {
                return Ok(0);
            }
            let n = self.inner.read(&mut buf[..to_read])?;
            if n > 0 {
                self.crc.slurp_block(&buf[..n]);
            }
            return Ok(n);
        }

        let n = self.inner.read(buf)?;
        if n > 0 {
            self.crc.slurp_block(&buf[..n]);
        }
        Ok(n)
    }
}

impl<T: Write> Write for CrcCalculatorStream<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        if n > 0 {
            self.crc.slurp_block(&buf[..n]);
        }
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_stream_write_updates_crc() {
        let mut out = Vec::new();
        {
            let mut s = CrcCalculatorStream::new(&mut out);
            s.write_all(b"123456789").unwrap();
            assert_eq!(s.total_bytes_slurped(), 9);
            assert_eq!(s.crc_u32(), 0xCBF43926);
        }
        assert_eq!(&out, b"123456789");
    }

    #[test]
    fn crc_stream_read_respects_limit() {
        let data = b"abcdefghij";
        let mut cur = std::io::Cursor::new(data.as_slice());
        let mut s = CrcCalculatorStream::with_length(&mut cur, 4);
        let mut buf = [0u8; 10];
        let n1 = s.read(&mut buf).unwrap();
        let n2 = s.read(&mut buf).unwrap();
        assert_eq!(n1, 4);
        assert_eq!(n2, 0);
        assert_eq!(s.total_bytes_slurped(), 4);
        assert_eq!(s.crc_u32(), crc32fast::hash(&data[..4]));
    }
}

