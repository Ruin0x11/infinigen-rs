use std::collections::{hash_map, HashMap};

use cell::Cell;
use chunk::*;
use dude::Dude;
use point::Point;

pub type WorldPosition = Point;

impl WorldPosition {
    pub fn from_chunk_index(index: ChunkIndex) -> Point {
        Point::new(index.0.x * CHUNK_SIZE, index.0.y * CHUNK_SIZE)
    }
}

pub struct World {
    chunks: HashMap<ChunkIndex, Chunk>,
    dudes: HashMap<WorldPosition, Dude>,
}

impl World {
    pub fn new(size: Point) -> Self {
        let chunks = World::generate_chunks(size.x, size.y);

        World {
            chunks: chunks,
            dudes: HashMap::new(),
        }
    }

    pub fn new_empty() -> Self {
        World {
            chunks: HashMap::new(),
            dudes: HashMap::new(),
        }
    }

    fn generate_chunks(width: i32, height: i32) -> HashMap<ChunkIndex, Chunk> {
        assert!(width > 0);
        assert!(height > 0);

        let mut chunks = HashMap::new();

        let ceiling = |q: i32, d: i32| (q + d - 1) / d;
        let columns = ceiling(width, CHUNK_SIZE);
        let rows = ceiling(height, CHUNK_SIZE);

        for i in 0..columns {
            for j in 0..rows {
                // let index = (j + (i * rows)) as usize;
                chunks.insert(ChunkIndex::new(i, j), Chunk::new(Cell::Floor));
            }
        }
        chunks
    }

    pub fn chunk_from_world_pos(&self, pos: WorldPosition) -> Option<&Chunk> {
        let index = ChunkIndex::from_world_pos(pos);
        self.chunk(index)
    }

    pub fn chunk_mut_from_world_pos(&mut self, pos: WorldPosition) -> Option<&mut Chunk> {
        let index = ChunkIndex::from_world_pos(pos);
        self.chunk_mut(index)
    }

    pub fn chunk(&self, index: ChunkIndex) -> Option<&Chunk> {
        self.chunks.get(&index)
    }

    pub fn chunk_mut(&mut self, index: ChunkIndex) -> Option<&mut Chunk> {
        self.chunks.get_mut(&index)
    }

    /// Return an iterator over `Cell` that covers a rectangular shape
    /// specified by the top-left (inclusive) point and the dimensions
    /// (width, height) of the rectangle.
    ///
    /// The iteration order is not specified.
    pub fn with_cells<F>(&mut self, top_left: WorldPosition, dimensions: Point, mut callback: F)
        where F: FnMut(Point, &Cell)
    {
        assert!(dimensions.x >= 0);
        assert!(dimensions.y >= 0);

        let mut chunk_index = ChunkIndex::from_world_pos(top_left);
        let mut world_pos = WorldPosition::from_chunk_index(chunk_index);
        let bottom_right = top_left + dimensions;
        let starter_chunk_x = world_pos.x;

        while world_pos.y < bottom_right.y {
            while world_pos.x < bottom_right.x {
                {
                    chunk_index = ChunkIndex::from_world_pos(world_pos);
                    let chunk_opt = self.chunk_from_world_pos(world_pos);
                    if let Some(chunk) = chunk_opt {
                        for (chunk_pos, cell) in chunk.iter() {
                            let cell_world_pos = chunk.world_position(&chunk_index, &chunk_pos);
                            if cell_world_pos >= top_left && cell_world_pos < bottom_right {
                                callback(cell_world_pos, cell);
                            }
                        }
                    }
                }
                world_pos.x += CHUNK_SIZE;
            }
            world_pos.y += CHUNK_SIZE;
            world_pos.x = starter_chunk_x;
        }

    }
}

impl World {
    pub fn place_dude(&mut self, pos: WorldPosition) {
        self.dudes.insert(pos, Dude::new(pos.clone()));
    }

    pub fn dudes(&mut self) -> hash_map::Values<WorldPosition, Dude> {
        self.dudes.values()
    }
}

use std::io;

#[derive(Debug)]
pub enum SerialError {
    NoChunkInWorld(ChunkIndex),
    NoChunkInSavefile(ChunkIndex),
    IoError(io::Error),
    EncodingError(bincode::ErrorKind),
}

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

pub use self::SerialError::*;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Read, Write};
use std::path::Path;
use std::mem;
use serial_chunk::*;
use bincode::{self, Infinite};

const SECTOR_SIZE: usize = 4096;
const LOOKUP_TABLE_SIZE: u64 = 256 * 2;

pub struct Region {
    handle: Box<File>,
}

/// Pads the given byte vec with zeroes to the next multiple of the given sector
/// size.
fn pad_byte_vec(bytes: &mut Vec<u8>, size: usize) {
    for _ in 0..(size - (bytes.len() % size)) {
        bytes.push(0);
    }
}

impl Region {
    pub fn write_chunk(&mut self, chunk: SerialChunk, index: &ChunkIndex) -> Result<(), SerialError>{
        let mut encoded: Vec<u8> = bincode::serialize(&chunk, Infinite)?;
        pad_byte_vec(&mut encoded, SECTOR_SIZE);

        let (offset, size) = self.read_chunk_offset(&index);

        match size {
            Some(size) => {
                assert!(size >= encoded.len(), "Chunk data grew past allocated sector_count!");
                self.update_chunk(encoded, offset)
            },
            None       => self.append_chunk(encoded, index),
        }
    }

    fn append_chunk(&mut self, encoded: Vec<u8>, index: &ChunkIndex) -> Result<(), SerialError> {
        println!("append");
        let sector_count: u16 = (encoded.len() / SECTOR_SIZE) as u16;

        let new_offset = self.handle.seek(SeekFrom::End(0))?;
        self.handle.write(encoded.as_slice())?;

        let val = Region::create_lookup_table_entry(new_offset, sector_count);
        self.write_chunk_offset(index, val)
    }

    fn update_chunk(&mut self, encoded: Vec<u8>, byte_offset: u64) -> Result<(), SerialError> {
        println!("update: bytes {}", byte_offset);
        self.handle.seek(SeekFrom::Start(LOOKUP_TABLE_SIZE + byte_offset))?;
        self.handle.write(encoded.as_slice())?;
        Ok(())
    }

    fn create_lookup_table_entry(eof: u64, sector_count: u16) -> u16 {
        let offset: u16 = (eof / SECTOR_SIZE as u64) as u16;
        println!("LOOKUP: {}", offset);

        offset | (sector_count << 8)
    }

    pub fn read_chunk(&mut self, index: &ChunkIndex) -> Result<SerialChunk, SerialError> {
        let (offset, size_opt) = self.read_chunk_offset(&index);
        let size = match size_opt {
            Some(s) => s,
            None    => return Err(NoChunkInSavefile(index.clone())),
        };

        println!("READ: {} {}", offset, size);

        let true_offset = LOOKUP_TABLE_SIZE + offset;
        let buf = self.read_bytes(true_offset, size);

        match bincode::deserialize(buf.as_slice()) {
            Ok(dat) => Ok(dat),
            Err(e)  => Err(SerialError::from(e)),
        }
    }

    fn read_bytes(&mut self, offset: u64, size: usize) -> Vec<u8> {
        self.handle.seek(SeekFrom::Start(offset)).unwrap();
        println!("offset {}", offset);
        let mut buf = vec![0u8; size];
        self.handle.read(buf.as_mut_slice()).unwrap();
        buf
    }

    fn get_filename(index: &ChunkIndex) -> String {
        format!("r.{}.{}.sr", (index.0.x as f32 / 16.0).floor(), (index.0.y as f32 / 16.0).floor())
    }

    fn get_chunk_offset(index: &ChunkIndex) -> u64 {
        2 * ((index.0.x % 16) + ((index.0.y % 16) * 16)) as u64
    }

    fn read_chunk_offset(&mut self, index: &ChunkIndex) -> (u64, Option<usize>) {
        // TODO: Handle negativity
        let offset = Region::get_chunk_offset(index);
        let data = self.read_bytes(offset, 2);

        // the byte offset should be u64 for Seek::seek, otherwise it will just
        // be cast every time.
        let offset = (data[0] as usize * SECTOR_SIZE) as u64;
        let size = if data[1] == 0 {
            None
        } else {
            Some(data[1] as usize * SECTOR_SIZE)
        };
        println!("offset: {} size: {:?}", offset, size);
        (offset, size)
    }

    fn write_chunk_offset(&mut self, index: &ChunkIndex, val: u16) -> Result<(), SerialError> {
        println!("Write chunk offset");
        // TODO: Handle negativity
        let offset = Region::get_chunk_offset(index);
        self.handle.seek(SeekFrom::Start(offset))?;
        let mut buf: [u8; 2] = unsafe { mem::transmute(val) };
        self.handle.write(&mut buf)?;
        Ok(())
    }

}

impl World {
    fn remove_dudes_in_chunk(&mut self, chunk_index: &ChunkIndex, chunk: &Chunk) -> HashMap<WorldPosition, Dude>
    {
        let mut dudes = HashMap::new();
        for (chunk_pos, _) in chunk.iter() {
            let cell_world_pos = chunk.world_position(chunk_index, &chunk_pos);
            if let Some(dude) = self.dudes.remove(&cell_world_pos) {
                dudes.insert(cell_world_pos, dude);
            }
        }
        dudes
    }

    // FIXME: inconsistent api, one returns a value but not the other
    pub fn load_chunk(&mut self, region: &mut Region, index: ChunkIndex) -> Result<(), SerialError> {
        let chunk = match region.read_chunk(&index) {
            Ok(c) => c,
            Err(e) => return Err(e),
        };
        println!("{:?}", chunk.dudes);

        for (pos, dude) in chunk.dudes.into_iter() {
            println!("dude!");
            self.dudes.insert(pos, dude);
        }
        self.chunks.insert(index, chunk.chunk);

        Ok(())
    }

    pub fn save_chunk(&mut self, region: &mut Region, index: &ChunkIndex) -> Result<(), SerialError> {
        let chunk = match self.unload_chunk(index) {
            Ok(c) => c,
            Err(e) => return Err(e),
        };
        region.write_chunk(chunk, index)
    }

    pub fn unload_chunk(&mut self, index: &ChunkIndex) -> Result<SerialChunk, SerialError> {
        if !self.chunks.contains_key(&index) {
            return Err(NoChunkInWorld(index.clone()));
        }
        let chunk = self.chunks.remove(&index).unwrap();
        let dudes = self.remove_dudes_in_chunk(&index, &chunk);
        let serial = SerialChunk {
            chunk: chunk,
            dudes: dudes,
        };
        Ok(serial)
    }

    fn get_region(index: &ChunkIndex) -> Region {
        let filename = Region::get_filename(index);
        if !Path::new(&filename).exists() {
            println!("create! {}", filename);
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(filename).unwrap();
            file.write(&[0u8; 256 * 2]).unwrap();
            Region {
                handle: Box::new(file),
            }
        } else {
            println!("Found! {}", filename);
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(filename).unwrap();
            Region {
                handle: Box::new(file),
            }
        }
    }

    pub fn save(mut self) -> Result<(), SerialError> {
        let filename = Region::get_filename(&ChunkIndex::new(0, 0)); // FIXME

        ::std::fs::remove_file(filename);

        let indices: Vec<ChunkIndex> = self.chunks.iter().map(|(&i, _)| i).collect();
        for index in indices.iter() {
            let mut region = World::get_region(&index);
            self.save_chunk(&mut region, index)?;
        }
        Ok(())
    }

    pub fn load() -> Result<Self, SerialError> {
        let index = ChunkIndex::new(0, 0);
        let mut region = World::get_region(&index);
        let mut world = World::new_empty();
        world.load_chunk(&mut region, index)?;
        Ok(world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saveload() {
        let mut world = World::new(Point::new(128, 128));
        let index = ChunkIndex::new(0, 0);
        let mut region = World::get_region(&index);

        world.place_dude(Point::new(0, 0));
        let count_before = world.dudes.len();

        world.save_chunk(&mut region, &index).unwrap();
        world.load_chunk(&mut region, index).unwrap();

        assert_eq!(count_before, world.dudes.len());
    }
}
