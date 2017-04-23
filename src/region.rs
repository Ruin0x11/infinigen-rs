use std::collections::{HashSet, hash_map, HashMap};
use std::fs::{File, OpenOptions};
use std::fmt;
use std::io::{self, Seek, SeekFrom, Read, Write};
use std::io::prelude::*;
use std::mem;
use std::path::Path;

use bincode::{self, Infinite};
// use flate2::Compression;
// use flate2::write::ZlibEncoder;
// use flate2::read::ZlibDecoder;

use chunk::*;
use point::Point;
use serial_chunk::*;

pub use self::SerialError::*;

#[derive(Debug)]
pub enum SerialError {
    NoChunkInWorld(ChunkIndex),
    NoChunkInSavefile(RegionLocalIndex),
    ChunkAlreadyLoaded(ChunkIndex),
    IoError(io::Error),
    EncodingError(bincode::ErrorKind),
}

type SerialResult<T> = Result<T, SerialError>;

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

const SECTOR_SIZE: usize = 4096;

/// The number of chunks per row inside regions.
const REGION_WIDTH: i32 = 16;

/// The total number of chunks per region.
const REGION_SIZE: i32 = REGION_WIDTH * REGION_WIDTH;

const LOOKUP_TABLE_SIZE: u64 = REGION_SIZE as u64 * 2;

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct RegionLocalIndex(pub Point);

impl From<ChunkIndex> for RegionLocalIndex {
    fn from(c: ChunkIndex) -> RegionLocalIndex {
        RegionLocalIndex(Point::new(c.0.x, c.0.y))
    }
}

impl fmt::Display for RegionLocalIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct RegionIndex(pub Point);

impl RegionIndex {
    pub fn new(x: i32, y: i32) -> Self {
        RegionIndex(Point::new(x, y))
    }
}

impl fmt::Display for RegionIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct Region {
    handle: Box<File>,
    unsaved_chunks: HashSet<ChunkIndex>,
}

pub struct RegionManager {
    regions: HashMap<RegionIndex, Region>,
}

impl RegionManager {
    pub fn new() -> Self {
        RegionManager {
            regions: HashMap::new(),
        }
    }

    fn get_region_index(chunk_index: &ChunkIndex) -> RegionIndex {
        let conv = |mut q: i32, d: i32| {
            // Divide by a larger number to make sure that negative division is
            // handled properly. Chunk index (-1, -1) should map to region index
            // (-1, -1), but -1 / REGION_WIDTH = 0.
            if q < 0 {
                q -= REGION_WIDTH;
            }

            (q / d)
        };
        RegionIndex::new(conv(chunk_index.0.x, REGION_WIDTH),
                         conv(chunk_index.0.y, REGION_WIDTH))

    }

    pub fn get_for_chunk(&mut self, chunk_index: &ChunkIndex) -> &mut Region {
        let region_index = RegionManager::get_region_index(chunk_index);
        println!("Chunk: {} Region: {}", chunk_index, region_index);

        if !self.regions.contains_key(&region_index) {
            self.regions.insert(region_index.clone(), Region::load(region_index));
        }

        self.regions.get_mut(&region_index).unwrap()
    }

    pub fn iter_mut(&mut self) -> hash_map::ValuesMut<RegionIndex, Region> {
        self.regions.values_mut()
    }

    pub fn notify_chunk_creation(&mut self, chunk_index: &ChunkIndex) {
        let region = self.get_for_chunk(chunk_index);
        region.receive_created_chunk(chunk_index.clone());
    }

    pub fn prune_empty(&mut self) {
        let indices: Vec<RegionIndex> = self.regions.iter().map(|(i, _)| i).cloned().collect();
        for idx in indices {
            if self.regions.get(&idx).map_or(false, |r| r.is_empty()) {
                println!("UNLOAD REGION {}", idx);
                self.regions.remove(&idx);
            }
        }
    }
}

/// Pads the given byte vec with zeroes to the next multiple of the given sector
/// size.
fn pad_byte_vec(bytes: &mut Vec<u8>, size: usize) {
    for _ in 0..(size - (bytes.len() % size)) {
        bytes.push(0);
    }
}

// fn compress_data(bytes: &Vec<u8>) -> SerialResult<Vec<u8>> {
//     let mut e = ZlibEncoder::new(Vec::new(), Compression::Default);
//     e.write(bytes.as_slice())?;
//     e.finish().map_err(SerialError::from)
// }

// fn decompress_data(bytes: &Vec<u8>) -> SerialResult<Vec<u8>> {
//     let mut d = ZlibDecoder::new(bytes.as_slice());
//     let mut buf = Vec::new();
//     d.read(&mut buf).map_err(SerialError::from)?;
//     Ok(buf)
// }

impl Region {
    pub fn load(index: RegionIndex) -> Self {
        println!("LOAD REGION {}", index);
        let filename = Region::get_filename(&index);

        let handle = Region::get_region_file(filename);

        Region {
            handle: Box::new(handle),
            unsaved_chunks: HashSet::new(),
        }
    }

    fn get_region_file(filename: String) -> File {
        if !Path::new(&filename).exists() {
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(filename) .unwrap();
            file.write(&[0u8; LOOKUP_TABLE_SIZE as usize]).unwrap();
            file
        } else {
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(filename).unwrap()
        }
    }

    /// Obtain this chunk's index relative to this region's index.
    fn normalize_chunk_index(chunk_index: &ChunkIndex) -> RegionLocalIndex {
        let conv = |i: i32| {
            let i_new = i % REGION_WIDTH;
            if i_new < 0 {
                REGION_WIDTH + i_new
            } else {
                i_new
            }
        };
        RegionLocalIndex(Point::new(conv(chunk_index.0.x), conv(chunk_index.0.y)))
    }

    pub fn write_chunk(&mut self, chunk: SerialChunk, index: &ChunkIndex) -> SerialResult<()>{
        assert!(self.unsaved_chunks.contains(index));

        let mut encoded: Vec<u8> = bincode::serialize(&chunk, Infinite)?;
        // FIXME: Compression makes chunk unloading nondeterministic, because
        // there is no way to know the amount of padding added and the
        // decompressor treats the padding as part of the file.

        // let mut compressed = compress_data(&mut encoded)?;
        let mut compressed = encoded;
        pad_byte_vec(&mut compressed, SECTOR_SIZE);

        let normalized_idx = Region::normalize_chunk_index(index);

        let (offset, size) = self.read_chunk_offset(&normalized_idx);
        println!("WRITE idx: {} offset: {} exists: {}", normalized_idx, offset, size.is_some());

        match size {
            Some(size) => {
                assert!(size >= compressed.len(), "Chunk data grew past allocated sector_count!");
                self.update_chunk(compressed, offset)?;
            },
            None       => { self.append_chunk(compressed, &normalized_idx)?; },
        }
        self.unsaved_chunks.remove(index);
        Ok(())
    }

    fn append_chunk(&mut self, chunk_data: Vec<u8>, index: &RegionLocalIndex) -> SerialResult<()> {
        let sector_count = (chunk_data.len() as f32 / SECTOR_SIZE as f32).ceil() as u32;
        assert!(sector_count < 256, "Sector count overflow!");
        assert!(sector_count > 0, "Sector count zero! Len: {}", chunk_data.len());
        let sector_count = sector_count as u8;

        let new_offset = self.handle.seek(SeekFrom::End(0))?;
        println!("APPEND idx: {} offset: {}", index, new_offset);

        self.handle.write(chunk_data.as_slice())?;
        self.write_chunk_offset(index, new_offset, sector_count)?;

        let (o, v) = self.read_chunk_offset(index);
        assert_eq!(new_offset, o, "index: {} new: {} old: {}", index, new_offset, o);
        assert_eq!(sector_count as usize * SECTOR_SIZE, v.unwrap());
        Ok(())
    }

    fn update_chunk(&mut self, chunk_data: Vec<u8>, byte_offset: u64) -> SerialResult<()> {
        self.handle.seek(SeekFrom::Start(byte_offset))?;
        self.handle.write(chunk_data.as_slice())?;
        Ok(())
    }

    fn create_lookup_table_entry(eof: u64, sector_count: u8) -> [u8; 2] {
        let offset: u8 = ((eof - LOOKUP_TABLE_SIZE) / SECTOR_SIZE as u64) as u8;

        [offset, sector_count]
    }

    pub fn read_chunk(&mut self, index: &ChunkIndex) -> SerialResult<SerialChunk> {
        assert!(!self.unsaved_chunks.contains(index));

        let normalized_idx = Region::normalize_chunk_index(index);
        let (offset, size_opt) = self.read_chunk_offset(&normalized_idx);
        println!("OFFSET: {}", offset);
        let size = match size_opt {
            Some(s) => s,
            None    => return Err(NoChunkInSavefile(normalized_idx.clone())),
        };

        println!("READ idx: {} offset: {}", normalized_idx, offset);
        let buf = self.read_bytes(offset, size);

        // let decompressed = decompress_data(&buf)?;
        match bincode::deserialize(buf.as_slice()) {
            Ok(dat) => {
                self.unsaved_chunks.insert(index.clone());
                Ok(dat)
            },
            Err(e)  => Err(SerialError::from(e)),
        }
    }

    fn read_bytes(&mut self, offset: u64, size: usize) -> Vec<u8> {
        self.handle.seek(SeekFrom::Start(offset)).unwrap();
        let mut buf = vec![0u8; size];
        self.handle.read(buf.as_mut_slice()).unwrap();
        buf
    }

    fn get_filename(index: &RegionIndex) -> String {
        format!("r.{}.{}.sr", index.0.x, index.0.y)
    }

    fn get_chunk_offset(index: &RegionLocalIndex) -> u64 {
        2 * ((index.0.x % 16) + ((index.0.y % 16) * 16)) as u64
    }

    fn read_chunk_offset(&mut self, index: &RegionLocalIndex) -> (u64, Option<usize>) {
        // TODO: Handle negativity
        let offset = Region::get_chunk_offset(index);
        let data = self.read_bytes(offset, 2);

        // the byte offset should be u64 for Seek::seek, otherwise it will just
        // be cast every time.
        let offset = LOOKUP_TABLE_SIZE + (data[0] as usize * SECTOR_SIZE) as u64;
        let size = if data[1] == 0 {
            None
        } else {
            Some(data[1] as usize * SECTOR_SIZE)
        };
        println!("idx: {} offset: {} size: {}", index, offset, data[1]);
        (offset, size)
    }

    fn write_chunk_offset(&mut self, index: &RegionLocalIndex, new_offset: u64, sector_count: u8) -> SerialResult<()> {
        println!("offset: {} sectors: {}", new_offset, sector_count);
        let val = Region::create_lookup_table_entry(new_offset, sector_count);
        let offset = Region::get_chunk_offset(index);
        self.handle.seek(SeekFrom::Start(offset))?;
        self.handle.write(&val)?;
        Ok(())
    }

    /// Notifies this Region that a chunk was created, so that its lifetime
    /// should be tracked by the Region.
    fn receive_created_chunk(&mut self, index: ChunkIndex) {
        self.unsaved_chunks.insert(index);
    }

    fn is_empty(&self) -> bool {
        self.unsaved_chunks.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_region_index() {
        assert_eq!(RegionManager::get_region_index(&ChunkIndex::new(0, 0)), RegionIndex::new(0, 0));
        assert_eq!(RegionManager::get_region_index(&ChunkIndex::new(0, 8)), RegionIndex::new(0, 0));
        assert_eq!(RegionManager::get_region_index(&ChunkIndex::new(0, 17)), RegionIndex::new(0, 1));
        assert_eq!(RegionManager::get_region_index(&ChunkIndex::new(0, 16)), RegionIndex::new(0, 1));
        assert_eq!(RegionManager::get_region_index(&ChunkIndex::new(0, 15)), RegionIndex::new(0, 0));
        assert_eq!(RegionManager::get_region_index(&ChunkIndex::new(0, -1)), RegionIndex::new(0, -1));
    }

}
