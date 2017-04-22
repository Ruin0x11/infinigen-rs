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

#[derive(Debug)]
pub enum SerialError {
    NoSuchChunk(ChunkIndex),
    EncodingError,
    DecodingError,
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

struct Region {
    handle: Box<File>,
}

impl Region {
    pub fn write_chunk(&mut self, chunk: SerialChunk, index: &ChunkIndex) -> Result<(), SerialError>{
        let mut encoded: Vec<u8> = match bincode::serialize(&chunk, Infinite) {
            Ok(dat) => dat,
            Err(_) => return Err(EncodingError),
        };

        for _ in 0..(SECTOR_SIZE - (encoded.len() % SECTOR_SIZE)) {
            encoded.push(0);
        }
        let sectors: u8 = (encoded.len() / SECTOR_SIZE) as u8;

        let (data_offset, data_size) = self.read_chunk_offset(&index);
        println!("{} {} lookup", data_offset, data_size);
        assert!(data_size <= encoded.len(),
                "Chunk data grew past allocated sectors!");

        if data_size == 0 {
            // Chunk didn't exist in region file; append to end
            let new_offset = self.handle.seek(SeekFrom::End(0))
                .expect("Unsuccessful region file append!");
            let new_offset: u16 = (new_offset / SECTOR_SIZE as u64) as u16;
            self.handle.write(encoded.as_slice()).unwrap();
            let val: u16 = sectors as u16 | (new_offset << 8);
            self.write_chunk_offset(index, val);
            println!("New at {}", new_offset);
        } else {
            // Use existing offset in region file
            let byte_offset: u64 = (data_offset) as u64;
            self.handle.seek(SeekFrom::Start(LOOKUP_TABLE_SIZE + byte_offset)).unwrap();
            self.handle.write(encoded.as_slice()).unwrap();
            println!("Existing at {}", data_offset);
        }
        Ok(())
    }

    pub fn read_chunk(&mut self, index: &ChunkIndex) -> Result<SerialChunk, SerialError> {
        let (data_offset, data_size) = self.read_chunk_offset(&index);
        if data_size == 0 {
            return Err(NoSuchChunk(index.clone()));
        }

        self.handle.seek(SeekFrom::Start(LOOKUP_TABLE_SIZE + data_offset as u64)).unwrap();
        println!("offset {}", data_offset);
        let mut buf = vec![0u8; data_size];
        self.handle.read(buf.as_mut_slice()).unwrap();

        match bincode::deserialize(buf.as_slice()) {
            Ok(dat) => Ok(dat),
            Err(_)  => Err(DecodingError),
        }
    }

    fn get_filename(index: &ChunkIndex) -> String {
        format!("r.{}.{}.sr", (index.0.x as f32 / 16.0).floor(), (index.0.y as f32 / 16.0).floor())
    }

    fn get_chunk_offset(index: &ChunkIndex) -> u64 {
        2 * ((index.0.x % 16) + ((index.0.y % 16) * 16)) as u64
    }

    fn read_chunk_offset(&mut self, index: &ChunkIndex) -> (usize, usize) {
        // TODO: Handle negativity
        let offset = Region::get_chunk_offset(index);
        self.handle.seek(SeekFrom::Start(offset));
        let mut ret = [0u8; 2];
        self.handle.read(&mut ret).unwrap();
        let offset = ret[0] as usize * SECTOR_SIZE;
        let size = ret[1] as usize * SECTOR_SIZE;
        (offset, size)
    }

    fn write_chunk_offset(&mut self, index: &ChunkIndex, val: u16) {
        // TODO: Handle negativity
        let offset = Region::get_chunk_offset(index);
        self.handle.seek(SeekFrom::Start(offset));
        let mut buf: [u8; 2] = unsafe { mem::transmute(val) };
        self.handle.write(&mut buf).unwrap();
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

    pub fn unload_chunk(&mut self, index: ChunkIndex) -> Result<SerialChunk, SerialError> {
        if !self.chunks.contains_key(&index) {
            return Err(NoSuchChunk(index));
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
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(filename).unwrap();
            file.write(&[0u8; 256 * 2]);
            Region {
                handle: Box::new(file),
            }
        } else {
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
            let chunk = self.unload_chunk(index.clone())?;
            println!("{:?}", chunk.dudes);
            region.write_chunk(chunk, index)?;
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
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open("/tmp/test").unwrap();
        let mut region = Region {
            handle: Box::new(file)
        };
        world.unload_chunk(ChunkIndex::new(0, 0)).unwrap();
        region.read_chunk(&ChunkIndex::new(0, 0)).unwrap();
    }
}
