use std::io::prelude::*;
use std::io::SeekFrom;
use std::fs::{File, OpenOptions};
use std::path::Path;

use bincode::{self, Infinite};
use serde::Serialize;
use serde::de::Deserialize;

use managed_region::ManagedRegion;
use region::*;

pub trait Index {
    fn x(&self) -> i32;
    fn y(&self) -> i32;
}

/// Pads the given byte vec with zeroes to the next multiple of the given sector
/// size.
fn pad_byte_vec(bytes: &mut Vec<u8>, size: usize) {
    for _ in 0..(size - (bytes.len() % size)) {
        bytes.push(0);
    }
}

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
