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

/// Describes a struct that can load and unload parts of the world. Typically
/// used alongside a Manager for keeping track of unsaved chunks.
pub trait Chunked<'a, H, I, C, R>
    where I:Index,
          C: Serialize + Deserialize,
          H: Seek + Write + Read,
          R: ManagedRegion<'a, C, H, I> {
    fn load_chunk(&mut self, index: &I) -> SerialResult<()>;
    fn unload_chunk(&mut self, index: &I) -> SerialResult<()>;
    fn chunk_loaded(&self, index: &I) -> bool;
    fn chunk_indices(&self) -> Vec<I>;
    fn update_chunks(&mut self) -> SerialResult<()>;

    fn save(self) -> SerialResult<()>;
}

/// Describes a struct that is responsible for keeping track of multiple
/// ManagedRegions and retrieving the correct region for a given chunk index.
pub trait Manager<'a, C, H, I, R>
    where I:Index,
          C: Serialize + Deserialize,
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
