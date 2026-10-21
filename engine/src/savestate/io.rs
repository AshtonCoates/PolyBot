use super::LoadError;
use crate::state::Coords;

pub struct Reader<'a> {
    data: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8], pos: usize) -> Self {
        Reader { data, pos }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], LoadError> {
        if self.pos + n > self.data.len() {
            return Err(LoadError::Eof { pos: self.pos, need: n });
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn u8(&mut self) -> Result<u8, LoadError> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool, LoadError> {
        Ok(self.u8()? != 0)
    }
    pub fn u16(&mut self) -> Result<u16, LoadError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn i16(&mut self) -> Result<i16, LoadError> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32, LoadError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32, LoadError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn f32(&mut self) -> Result<f32, LoadError> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn coords(&mut self) -> Result<Coords, LoadError> {
        let x = self.i32()?;
        let y = self.i32()?;
        Ok(Coords { x, y })
    }

    /// .NET `BinaryReader.ReadString`: 7-bit encoded byte length, then UTF-8.
    pub fn string(&mut self) -> Result<String, LoadError> {
        let start = self.pos;
        let mut len: usize = 0;
        let mut shift = 0;
        loop {
            let b = self.u8()?;
            len |= ((b & 0x7f) as usize) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 28 {
                return Err(LoadError::BadString { pos: start });
            }
        }
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| LoadError::BadString { pos: start })
    }
}

#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn bool(&mut self, v: bool) {
        self.buf.push(v as u8);
    }
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i16(&mut self, v: i16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn coords(&mut self, c: Coords) {
        self.i32(c.x);
        self.i32(c.y);
    }
    pub fn string(&mut self, s: &str) {
        let mut len = s.len();
        loop {
            let b = (len & 0x7f) as u8;
            len >>= 7;
            if len == 0 {
                self.buf.push(b);
                break;
            }
            self.buf.push(b | 0x80);
        }
        self.buf.extend_from_slice(s.as_bytes());
    }
}
