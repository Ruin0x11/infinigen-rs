use std::hash::Hash;

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

/// Describes a struct that is responsible for keeping track of multiple
/// ManagedRegions and retrieving the correct region for a given chunk index.
pub trait RegionManager<'a, I, C>
    where I:Index,
          C: ManagedChunk,
          Region<I>: ManagedRegion<'a, I, C> {

    fn load(&mut self, index: RegionIndex);
    fn get(&mut self, index: &RegionIndex) -> Option<&Region<I>>;
    fn get_mut(&mut self, index: &RegionIndex) -> Option<&mut Region<I>>;
    fn remove(&mut self, index: &RegionIndex);
    fn region_loaded(&self, index: &RegionIndex) -> bool;
    fn region_indices(&self) -> Vec<RegionIndex>;

    fn notify_chunk_creation(&mut self, chunk_index: &I) {
        let region = self.get_for_chunk(chunk_index);
        region.receive_created_chunk(chunk_index);
    }

    fn prune_empty(&mut self) {
        let indices = self.region_indices();
        for idx in indices {
            if self.get(&idx).map_or(false, |r: &Region<I>| r.is_empty()) {
                self.remove(&idx);
            }
        }
    }

    fn get_for_chunk(&mut self, chunk_index: &I) -> &mut Region<I> {
        let region_index = Region::get_region_index(chunk_index);

        if !self.region_loaded(&region_index) {
            self.load(region_index);
        }

        self.get_mut(&region_index).unwrap()
    }
}

/// Describes a struct that can load and unload parts of the world. Used
/// alongside a Manager for keeping track of unsaved chunks.
pub trait ChunkedTerrain<'a, I, C, M>
    where I:Index,
          C: ManagedChunk,
          M: RegionManager<'a, I, C> {

    fn chunk_loaded(&self, index: &I) -> bool;
    fn chunk_indices(&self) -> Vec<I>;
    fn chunk_count(&self) -> usize;

    fn regions_mut(&mut self) -> &mut M;
}

pub trait ChunkedWorld<'a, I, C, M, T>
    where I: Index,
          C: ManagedChunk,
          M: RegionManager<'a, I, C>,
          T: ChunkedTerrain<'a, I, C, M> {
    fn load_chunk_internal(&mut self, chunk: C, index: &I) -> SerialResult<()>;
    fn unload_chunk_internal(&mut self, index: &I) -> SerialResult<C>;

    fn load_chunk_from_region(&mut self, index: &I) -> SerialResult<()> {
        let old_count = self.terrain().chunk_count();
        let chunk: C;
        {
            let region = self.terrain_mut().regions_mut().get_for_chunk(index);
            chunk = match region.read_chunk(index) {
                Ok(c) => c,
                Err(e) => return Err(e),
            };
        }

        self.load_chunk_internal(chunk, index)?;

        assert_eq!(self.terrain().chunk_count(), old_count + 1,
                   "Chunk wasn't inserted into world!");

        Ok(())
    }

    fn generate_chunk(&mut self, index: &I) -> SerialResult<()>;
    fn update_chunks(&mut self) -> SerialResult<()>;
    fn terrain(&self) -> &T;
    fn terrain_mut(&mut self) -> &mut T;
    fn save(&mut self) -> SerialResult<()>;

    fn load_chunk(&mut self, index: &I) -> SerialResult<()> {
        match self.load_chunk_from_region(index) {
            Err(SerialError::NoChunkInSavefile(_)) => {
                let old_count = self.terrain().chunk_count();
                if self.terrain().chunk_loaded(index) {
                    return Err(ChunkAlreadyLoaded(index.x(), index.y()));
                }

                self.generate_chunk(index)?;

                assert_eq!(self.terrain().chunk_count(), old_count + 1,
                           "Chunk wasn't inserted into world!");

                // The region this chunk was created in needs to know of the chunk
                // that was created in-game but nonexistent on disk.
                self.terrain_mut().regions_mut().notify_chunk_creation(index);
            },
            Err(e) => panic!("{:?}", e),
            Ok(()) => (),
        }
        Ok(())
    }

    fn unload_chunk(&mut self, index: &I) -> SerialResult<()> {
        let old_count = self.terrain().chunk_count();
        let chunk = match self.unload_chunk_internal(index) {
            Ok(c) => c,
            Err(e) => return Err(e),
        };

        assert_eq!(self.terrain().chunk_count(), old_count - 1,
                   "Chunk wasn't removed from world!");

        let region = self.terrain_mut().regions_mut().get_for_chunk(index);
        region.write_chunk(chunk, index)
    }
}
