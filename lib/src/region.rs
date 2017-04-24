use std::fmt;
use std::io;

use bincode;

pub use self::SerialError::*;

#[derive(Debug)]
pub enum SerialError {
    NoChunkInWorld(i32, i32),
    NoChunkInSavefile(RegionLocalIndex),
    ChunkAlreadyLoaded(i32, i32),
    IoError(io::Error),
    EncodingError(bincode::ErrorKind),
}

pub type SerialResult<T> = Result<T, SerialError>;

impl From<io::Error> for SerialError {
    fn from(e: io::Error) -> SerialError {
        IoError(e)
    }
}

impl From<Box<bincode::ErrorKind>> for SerialError {
    fn from(e: Box<bincode::ErrorKind>) -> SerialError {
        EncodingError(*e)
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct RegionLocalIndex(pub i32, pub i32);

impl fmt::Display for RegionLocalIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct RegionIndex(pub i32, pub i32);

impl fmt::Display for RegionIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}


#[cfg(never)]
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_region_index() {
        assert_eq!(util::get_region_index(&ChunkIndex::new(0, 0)), RegionIndex::new(0, 0));
        assert_eq!(util::get_region_index(&ChunkIndex::new(0, 8)), RegionIndex::new(0, 0));
        assert_eq!(util::get_region_index(&ChunkIndex::new(0, 17)), RegionIndex::new(0, 1));
        assert_eq!(util::get_region_index(&ChunkIndex::new(0, 16)), RegionIndex::new(0, 1));
        assert_eq!(util::get_region_index(&ChunkIndex::new(0, 15)), RegionIndex::new(0, 0));
        assert_eq!(util::get_region_index(&ChunkIndex::new(0, -1)), RegionIndex::new(0, -1));
    }

}
