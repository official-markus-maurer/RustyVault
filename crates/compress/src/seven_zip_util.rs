use std::io::{Read, Write};

pub fn mem_set(buffer: &mut [u8], start: usize, val: u8, len: usize) {
    let end = start.saturating_add(len).min(buffer.len());
    for b in &mut buffer[start..end] {
        *b = val;
    }
}

pub fn mem_crypt(
    dest: &mut [u8],
    dest_point: usize,
    source: &[u8],
    source_point: usize,
    len: usize,
) {
    if dest_point >= dest.len() || source_point >= source.len() {
        return;
    }
    let max_len = std::cmp::min(
        len,
        std::cmp::min(dest.len() - dest_point, source.len() - source_point),
    );
    for i in (0..max_len).rev() {
        dest[dest_point + i] = source[source_point + i];
    }
}

pub fn mem_cmp(buffer1: &[u8], offset: usize, buffer2: &[u8]) -> bool {
    if offset >= buffer1.len() {
        return false;
    }
    if buffer1.len() - offset < buffer2.len() {
        return false;
    }
    buffer1[offset..offset + buffer2.len()] == *buffer2
}

pub fn read_encoded_u64<R: Read>(r: &mut R) -> std::io::Result<u64> {
    let mut first = [0u8; 1];
    r.read_exact(&mut first)?;
    let first_byte = first[0];
    let mut mask = 0x80u8;
    let mut value: u64 = 0;
    for i in 0..8 {
        if (first_byte & mask) == 0 {
            let high_part = (first_byte & (mask - 1)) as u64;
            value = value.wrapping_add(high_part << (8 * i));
            return Ok(value);
        }
        let mut b = [0u8; 1];
        r.read_exact(&mut b)?;
        value |= (b[0] as u64) << (8 * i);
        mask >>= 1;
    }
    Ok(value)
}

pub fn write_encoded_u64<W: Write>(w: &mut W, mut value: u64) -> std::io::Result<()> {
    let mut first_byte: u8 = 0;
    let mut mask: u8 = 0x80;
    let mut i = 0usize;
    let mut broke = false;
    for idx in 0..8usize {
        if value < (1u64 << (7 * (idx + 1))) {
            first_byte |= (value >> (8 * idx)) as u8;
            i = idx;
            broke = true;
            break;
        }
        first_byte |= mask;
        mask >>= 1;
    }
    if !broke {
        i = 8;
    }
    w.write_all(&[first_byte])?;
    while i > 0 {
        w.write_all(&[(value & 0xFF) as u8])?;
        value >>= 8;
        i -= 1;
    }
    Ok(())
}

pub fn read_name_utf16le<R: Read>(r: &mut R) -> std::io::Result<String> {
    let mut u16s: Vec<u16> = Vec::new();
    loop {
        let mut b = [0u8; 2];
        r.read_exact(&mut b)?;
        let c = u16::from_le_bytes(b);
        if c == 0 {
            break;
        }
        u16s.push(c);
    }
    Ok(String::from_utf16_lossy(&u16s))
}

pub fn write_name_utf16le<W: Write>(w: &mut W, name: &str) -> std::io::Result<()> {
    for c in name.encode_utf16() {
        w.write_all(&c.to_le_bytes())?;
    }
    w.write_all(&0u16.to_le_bytes())?;
    Ok(())
}

pub fn read_bool_flags<R: Read>(r: &mut R, num_items: usize) -> std::io::Result<Vec<bool>> {
    let mut b = 0u8;
    let mut mask = 0u8;
    let mut flags = vec![false; num_items];
    for v in flags.iter_mut().take(num_items) {
        if mask == 0 {
            let mut tmp = [0u8; 1];
            r.read_exact(&mut tmp)?;
            b = tmp[0];
            mask = 0x80;
        }
        *v = (b & mask) != 0;
        mask >>= 1;
    }
    Ok(flags)
}

pub fn write_bool_flags<W: Write>(w: &mut W, b_array: &[bool]) -> std::io::Result<()> {
    let byte_count = b_array.len().div_ceil(8);
    write_encoded_u64(w, byte_count as u64)?;
    let mut mask = 0x80u8;
    let mut out = 0u8;
    for v in b_array {
        if *v {
            out |= mask;
        }
        mask >>= 1;
        if mask != 0 {
            continue;
        }
        w.write_all(&[out])?;
        mask = 0x80;
        out = 0;
    }
    if mask != 0x80 {
        w.write_all(&[out])?;
    }
    Ok(())
}

pub fn read_bool_flags_default_true<R: Read>(
    r: &mut R,
    num_items: usize,
) -> std::io::Result<Vec<bool>> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    if b[0] == 0 {
        return read_bool_flags(r, num_items);
    }
    Ok(vec![true; num_items])
}

pub fn write_bool_flags_default_true<W: Write>(w: &mut W, b_array: &[bool]) -> std::io::Result<()> {
    let all_true = b_array.iter().all(|b| *b);
    if all_true {
        w.write_all(&[1u8])?;
        return Ok(());
    }
    w.write_all(&[0u8])?;
    write_bool_flags(w, b_array)
}

pub fn unpack_crcs<R: Read>(r: &mut R, num_items: usize) -> std::io::Result<Vec<Option<u32>>> {
    let defined = read_bool_flags_default_true(r, num_items)?;
    let mut digests = vec![None; num_items];
    for i in 0..num_items {
        if defined[i] {
            let mut b = [0u8; 4];
            r.read_exact(&mut b)?;
            digests[i] = Some(u32::from_le_bytes(b));
        }
    }
    Ok(digests)
}

pub fn write_packed_crcs<W: Write>(w: &mut W, digests: &[Option<u32>]) -> std::io::Result<()> {
    let defined: Vec<bool> = digests.iter().map(|d| d.is_some()).collect();
    write_bool_flags_default_true(w, &defined)?;
    for (i, d) in digests.iter().enumerate() {
        if defined[i] {
            w.write_all(&d.unwrap().to_le_bytes())?;
        }
    }
    Ok(())
}

pub fn write_u32_def<W: Write>(w: &mut W, values: &[u32]) -> std::io::Result<()> {
    write_encoded_u64(w, (values.len() * 4 + 2) as u64)?;
    w.write_all(&[1u8, 0u8])?;
    for v in values {
        w.write_all(&v.to_le_bytes())?;
    }
    Ok(())
}

pub fn read_u32_def<R: Read>(r: &mut R, num_items: usize) -> std::io::Result<Vec<u32>> {
    let defs = read_bool_flags2(r, num_items)?;
    let mut tmp = [0u8; 1];
    r.read_exact(&mut tmp)?;
    let _ = tmp[0];
    let mut out = vec![0u32; num_items];
    for i in 0..num_items {
        if defs[i] {
            let mut b = [0u8; 4];
            r.read_exact(&mut b)?;
            out[i] = u32::from_le_bytes(b);
        }
    }
    Ok(out)
}

pub fn read_u64_def<R: Read>(r: &mut R, num_items: usize) -> std::io::Result<Vec<u64>> {
    let defs = read_bool_flags2(r, num_items)?;
    let mut tmp = [0u8; 1];
    r.read_exact(&mut tmp)?;
    let _ = tmp[0];
    let mut out = vec![0u64; num_items];
    for i in 0..num_items {
        if defs[i] {
            let mut b = [0u8; 8];
            r.read_exact(&mut b)?;
            out[i] = u64::from_le_bytes(b);
        }
    }
    Ok(out)
}

pub fn read_bool_flags2<R: Read>(r: &mut R, num_items: usize) -> std::io::Result<Vec<bool>> {
    let mut all = [0u8; 1];
    r.read_exact(&mut all)?;
    if all[0] == 0 {
        return read_bool_flags(r, num_items);
    }
    Ok(vec![true; num_items])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_u64_roundtrip() {
        let values = [0u64, 1, 0x7F, 0x80, 0x1234, 0x12345678, u64::MAX / 2];
        for v in values {
            let mut buf = Vec::new();
            write_encoded_u64(&mut buf, v).unwrap();
            let mut cur = std::io::Cursor::new(buf);
            let got = read_encoded_u64(&mut cur).unwrap();
            assert_eq!(got, v);
        }
    }

    #[test]
    fn name_roundtrip() {
        let s = "abcΩ";
        let mut buf = Vec::new();
        write_name_utf16le(&mut buf, s).unwrap();
        let mut cur = std::io::Cursor::new(buf);
        let got = read_name_utf16le(&mut cur).unwrap();
        assert_eq!(got, s);
    }
}
