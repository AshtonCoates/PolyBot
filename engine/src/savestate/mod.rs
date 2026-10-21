//! Reading (and, for round-trip testing, writing) Polytopia `.state` files.
//!
//! File = LZ4 pickle (`lz4`) around a .NET BinaryWriter payload (`codec`); see docs/game-findings.md.

mod codec;
mod io;
mod lz4;

use std::fmt;

use crate::state::LocalGameData;

pub use codec::{read_local_game_data, write_local_game_data};
pub use lz4::unpickle;

#[derive(Debug)]
pub enum LoadError {
    /// Ran out of bytes.
    Eof { pos: usize, need: usize },
    /// Invalid UTF-8 or string length.
    BadString { pos: usize },
    /// Corrupt LZ4 data.
    Lz4(String),
    /// A layout we have not decoded (older version, unknown command type, non-empty action stack, ...).
    Unsupported(String),
    /// Bytes left over after the last record.
    Trailing { pos: usize, len: usize },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Eof { pos, need } => write!(f, "unexpected end of data at {pos} (needed {need} bytes)"),
            LoadError::BadString { pos } => write!(f, "invalid string at {pos}"),
            LoadError::Lz4(m) => write!(f, "lz4: {m}"),
            LoadError::Unsupported(m) => write!(f, "unsupported: {m}"),
            LoadError::Trailing { pos, len } => write!(f, "{} unparsed bytes at {pos}", len - pos),
        }
    }
}

impl std::error::Error for LoadError {}

/// Parse the raw bytes of a `.state` file.
pub fn load_state_file(file_bytes: &[u8]) -> Result<LocalGameData, LoadError> {
    read_local_game_data(&unpickle(file_bytes)?)
}
