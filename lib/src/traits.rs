use std::hash::Hash;
use std::io::prelude::*;

use serde::Serialize;
use serde::de::Deserialize;

use managed_region::ManagedRegion;
use region::*;

/// A two-dimensional index into a grid, like those of chunks or regions.
pub trait Index: Hash + Eq + PartialEq + Clone {
    fn x(&self) -> i32;
    fn y(&self) -> i32;
}

/// Allows the user to specify the parameters of the region fil
pub trait ManagedChunk: Serialize + Deserialize {
       /// The number of bytes to align the saved chunk data to in the region file.
    /// Should be a power of two.
    const SECTOR_SIZE: usize = 4096;

    /// The number of chunks per row inside regions.
    const REGION_WIDTH: i32 = 16;
}

/// Describes a struct that can load and unload parts of the world. Used
/// alongside a Manager for keeping track of unsaved chunks.
pub trait Chunked<'a, H, I, C, R, M>
    where I:Index,
          C: ManagedChunk,
          H: Seek + Write + Read,
          R: ManagedRegion<'a, C, H, I>,
          M: Manager<'a, C, H, I, R> {

    fn load_chunk_internal(&mut self, chunk: C, index: &I) -> SerialResult<()>;
    fn unload_chunk_internal(&mut self, index: &I) -> SerialResult<C>;
    fn generate_chunk(&mut self, index: &I) -> SerialResult<()>;
    fn update_chunks(&mut self) -> SerialResult<()>;

    fn chunk_loaded(&self, index: &I) -> bool;
    fn chunk_indices(&self) -> Vec<I>;
    fn chunk_count(&self) -> usize;

    fn regions_mut(&mut self) -> &mut M;

    fn save(self) -> SerialResult<()>;

    fn load_chunk(&mut self, index: &I) -> SerialResult<()> {
        if let Err(_) = self.load_chunk_from_region(index) {
            let old_count = self.chunk_count();
            if self.chunk_loaded(index) {
                return Err(ChunkAlreadyLoaded(index.x(), index.y()));
            }

            self.generate_chunk(index)?;

            assert_eq!(self.chunk_count(), old_count + 1,
                       "Chunk wasn't inserted into world!");

            // The region this chunk was created in needs to know of the chunk
            // that was created in-game but nonexistent on disk.
            self.regions_mut().notify_chunk_creation(index);
        }
        Ok(())
    }

    fn load_chunk_from_region(&mut self, index: &I) -> SerialResult<()> {
        let old_count = self.chunk_count();
        let chunk: C;
        {
            let region = self.regions_mut().get_for_chunk(index);
            chunk = match region.read_chunk(index) {
                Ok(c) => c,
                Err(e) => return Err(e),
            };
        }

        self.load_chunk_internal(chunk, index)?;

        assert_eq!(self.chunk_count(), old_count + 1,
                   "Chunk wasn't inserted into world!");

        Ok(())
    }

    fn unload_chunk(&mut self, index: &I) -> SerialResult<()> {
        let old_count = self.chunk_count();
        let chunk = match self.unload_chunk_internal(index) {
            Ok(c) => c,
            Err(e) => return Err(e),
        };

        assert_eq!(self.chunk_count(), old_count - 1,
                   "Chunk wasn't removed from world!");

        let region = self.regions_mut().get_for_chunk(index);
        region.write_chunk(chunk, index)
    }
}

/// Describes a struct that is responsible for keeping track of multiple
/// ManagedRegions and retrieving the correct region for a given chunk index.
pub trait Manager<'a, C, H, I, R>
    where I:Index,
          C: ManagedChunk,
          H: Seek + Read + Write,
          R: ManagedRegion<'a, C, H, I> {

    fn load(&self, index: RegionIndex) -> R;
    fn get_for_chunk(&mut self, chunk_index: &I) -> &mut R;
    fn prune_empty(&mut self);

    fn notify_chunk_creation(&mut self, chunk_index: &I) {
        let region = self.get_for_chunk(chunk_index);
        region.receive_created_chunk(chunk_index);
    }
}
