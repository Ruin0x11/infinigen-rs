use std::collections::HashSet;
use std::fmt;
use std::io;
use std::fs::File;

use bincode;

use traits::{Index, ManagedChunk};
use managed_region::ManagedRegion;

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

/// An index of a chunk inside a region's coordinate space.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct RegionLocalIndex(pub i32, pub i32);

impl fmt::Display for RegionLocalIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An index of a region in a grid of all regions.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct RegionIndex(pub i32, pub i32);

impl fmt::Display for RegionIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Implementation of a region for on-disk serialization.
pub struct Region<I: Index> {
    pub handle: Box<File>,
    pub unsaved_chunks: HashSet<I>,
}

impl<'a, I: Index, C: ManagedChunk> ManagedRegion<'a, C, I> for Region<I> {
    fn handle(&mut self) -> &mut File {
        &mut self.handle
    }

    fn mark_as_saved(&mut self, index: &I) {
        self.unsaved_chunks.remove(index);
    }

    fn mark_as_unsaved(&mut self, index: &I) {
        self.unsaved_chunks.insert(index.clone());
    }

    fn chunk_unsaved(&self, index: &I) -> bool {
        self.unsaved_chunks.contains(index)
    }

    fn receive_created_chunk(&mut self, index: &I) {
        self.unsaved_chunks.insert(index.clone());
    }

    fn is_empty(&self) -> bool {
        self.unsaved_chunks.len() == 0
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
