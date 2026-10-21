//! K4os.Compression.LZ4 "pickle" decoding.
//!
//! Header byte: bits 0-2 = version (0), bits 6-7 = size-of-diff code (3 means 4 bytes); then that many
//! little-endian bytes holding (uncompressed length - compressed length), then a raw LZ4 block.
//! A diff of 0 means the data is stored uncompressed.

use super::LoadError;

pub fn unpickle(data: &[u8]) -> Result<Vec<u8>, LoadError> {
    let header = *data.first().ok_or_else(|| LoadError::Lz4("empty input".into()))?;
    if header & 0x07 != 0 {
        return Err(LoadError::Lz4(format!("unknown pickle version {}", header & 0x07)));
    }
    let code = (header >> 6) & 0x3;
    let diff_size = if code == 3 { 4 } else { code as usize };
    let data_offset = 1 + diff_size;
    if data.len() < data_offset {
        return Err(LoadError::Lz4("truncated header".into()));
    }
    let mut diff = 0usize;
    for (i, b) in data[1..data_offset].iter().enumerate() {
        diff |= (*b as usize) << (8 * i);
    }
    let body = &data[data_offset..];
    if diff == 0 {
        return Ok(body.to_vec());
    }
    block_decode(body, body.len() + diff)
}

pub fn block_decode(src: &[u8], expected: usize) -> Result<Vec<u8>, LoadError> {
    let err = |m: &str| LoadError::Lz4(m.to_string());
    let mut out: Vec<u8> = Vec::with_capacity(expected);
    let mut i = 0;
    while i < src.len() {
        let token = src[i];
        i += 1;
        let mut lit = (token >> 4) as usize;
        if lit == 15 {
            loop {
                let b = *src.get(i).ok_or_else(|| err("truncated literal length"))?;
                i += 1;
                lit += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        let lits = src.get(i..i + lit).ok_or_else(|| err("truncated literals"))?;
        out.extend_from_slice(lits);
        i += lit;
        if i >= src.len() {
            break; // the last sequence carries literals only
        }
        let offset = *src.get(i).ok_or_else(|| err("truncated offset"))? as usize
            | (*src.get(i + 1).ok_or_else(|| err("truncated offset"))? as usize) << 8;
        i += 2;
        if offset == 0 || offset > out.len() {
            return Err(err("invalid match offset"));
        }
        let mut mlen = (token & 0xf) as usize;
        if mlen == 15 {
            loop {
                let b = *src.get(i).ok_or_else(|| err("truncated match length"))?;
                i += 1;
                mlen += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        mlen += 4;
        let start = out.len() - offset;
        for k in 0..mlen {
            let b = out[start + k]; // byte-wise: a match may overlap its own output
            out.push(b);
        }
    }
    if out.len() != expected {
        return Err(LoadError::Lz4(format!("decoded {} bytes, expected {}", out.len(), expected)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_literal_then_overlapping_match() {
        // literal 'a', match offset 1 length 9, then literal-only tail 'b'
        let block = [0x15, b'a', 0x01, 0x00, 0x10, b'b'];
        assert_eq!(block_decode(&block, 11).unwrap(), b"aaaaaaaaaab");
    }

    #[test]
    fn uncompressed_pickle_passes_through() {
        assert_eq!(unpickle(&[0x00, 1, 2, 3]).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn wrong_length_is_an_error() {
        let block = [0x15, b'a', 0x01, 0x00, 0x10, b'b'];
        assert!(block_decode(&block, 12).is_err());
    }
}
