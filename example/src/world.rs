use std::collections::{HashSet, hash_map, HashMap};
use std::fs::File;
use std::io::{Seek, SeekFrom, Read};

use noise::{Perlin, Seedable};
use infinigen::*;
use serde::Serialize;
use serde::de::Deserialize;

use cell::Cell;
use chunk::*;
use dude::Dude;
use point::Point;

pub struct MyRegion {
    pub handle: Box<File>,
    pub unsaved_chunks: HashSet<ChunkIndex>,
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

impl<'a, C: Serialize + Deserialize> ManagedRegion<'a, C, File, ChunkIndex> for MyRegion {
    fn handle(&mut self) -> &mut File {
        &mut self.handle
    }

    fn mark_as_saved(&mut self, index: &ChunkIndex) {
        self.unsaved_chunks.remove(index);
    }

    fn mark_as_unsaved(&mut self, index: &ChunkIndex) {
        self.unsaved_chunks.insert(index.clone());
    }

    fn chunk_unsaved(&self, index: &ChunkIndex) -> bool {
        self.unsaved_chunks.contains(index)
    }

    fn receive_created_chunk(&mut self, index: &ChunkIndex) {
        self.unsaved_chunks.insert(index.clone());
    }

    fn read_bytes(&mut self, offset: u64, size: usize) -> Vec<u8> {
        self.handle.seek(SeekFrom::Start(offset)).unwrap();
        let mut buf = vec![0u8; size];
        self.handle.read(buf.as_mut_slice()).unwrap();
        buf
    }

    fn is_empty(&self) -> bool {
        self.unsaved_chunks.len() == 0
    }
}

pub struct RegionManager {
    regions: HashMap<RegionIndex, MyRegion>,
}

impl RegionManager {
    pub fn new() -> Self {
        RegionManager {
            regions: HashMap::new(),
        }
    }
}

// TODO: Is there some way of using AsRef here instead, because we don't care
// about the underlying 2D point struct?
impl Index for ChunkIndex {
    fn x(&self) -> i32 { self.0.x }
    fn y(&self) -> i32 { self.0.y }
}

pub fn get_filename(index: &RegionIndex) -> String {
    format!("r.{}.{}.sr", index.0, index.1)
}

impl<'a> Manager<'a, SerialChunk, File, ChunkIndex, MyRegion> for RegionManager
    where MyRegion: ManagedRegion<'a, SerialChunk, File, ChunkIndex>{
    fn load(&self, index: RegionIndex) -> MyRegion {
        println!("LOAD REGION {}", index);
        let filename = get_filename(&index);

        let handle = MyRegion::get_region_file(filename);

        MyRegion {
            handle: Box::new(handle),
            unsaved_chunks: HashSet::new(),
        }
    }

    fn prune_empty(&mut self) {
        let indices: Vec<RegionIndex> = self.regions.iter().map(|(i, _)| i).cloned().collect();
        for idx in indices {
            if self.regions.get(&idx).map_or(false, |r: &MyRegion| r.is_empty()) {
                println!("UNLOAD REGION {}", idx);
                self.regions.remove(&idx);
            }
        }
    }


    fn get_for_chunk(&mut self, chunk_index: &ChunkIndex) -> &mut MyRegion {
        let region_index = MyRegion::get_region_index(chunk_index);

        if !self.regions.contains_key(&region_index) {
            let region = self.load(region_index);
            self.regions.insert(region_index.clone(), region);
        }

        self.regions.get_mut(&region_index).unwrap()
    }
}

pub type WorldPosition = Point;

impl WorldPosition {
    pub fn from_chunk_index(index: ChunkIndex) -> Point {
        Point::new(index.0.x * CHUNK_WIDTH, index.0.y * CHUNK_WIDTH)
    }
}

pub struct World {
    regions: RegionManager,
    chunks: HashMap<ChunkIndex, Chunk>,
    dudes: HashMap<WorldPosition, Dude>,
    pub observer: WorldPosition,

    gen: Perlin,
}

impl World {
    pub fn new_empty() -> Self {
        World {
            regions: RegionManager::new(),
            chunks: HashMap::new(),
            dudes: HashMap::new(),
            observer: WorldPosition::new(0, 0),

            // TODO: Save world information, seed
            gen: Perlin::new().set_seed(2),
        }
    }

    pub fn chunk_from_world_pos(&self, pos: WorldPosition) -> Option<&Chunk> {
        let index = ChunkIndex::from_world_pos(pos);
        self.chunk(index)
    }

    pub fn chunk(&self, index: ChunkIndex) -> Option<&Chunk> {
        self.chunks.get(&index)
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
                world_pos.x += CHUNK_WIDTH;
            }
            world_pos.y += CHUNK_WIDTH;
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
}

impl World {
    pub fn load_chunk_from_save(&mut self, index: &ChunkIndex) -> Result<(), SerialError> {
        let region = self.regions.get_for_chunk(index);
        let chunk: SerialChunk = match region.read_chunk(index) {
            Ok(c) => c,
            Err(e) => return Err(e),
        };
        println!("Loading chunk at {}", index);
        for (pos, dude) in chunk.dudes.into_iter() {
            println!("dude!");
            self.dudes.insert(pos, dude);
        }
        self.chunks.insert(index.clone(), chunk.chunk);

        Ok(())
    }

    fn unload_chunk_internal(&mut self, index: &ChunkIndex) -> Result<SerialChunk, SerialError> {
        let chunk = match self.chunks.remove(&index) {
            Some(c) => c,
            None => return Err(NoChunkInWorld(index.0.x, index.0.y)),
        };
        let dudes = self.remove_dudes_in_chunk(&index, &chunk);
        println!("Unloading chunk at {}", index);
        let serial = SerialChunk {
            chunk: chunk,
            dudes: dudes,
        };
        Ok(serial)
    }
}

const UPDATE_RADIUS: i32 = 3;

impl<'a> Chunked<'a, File, ChunkIndex, SerialChunk, MyRegion> for World {
    fn load_chunk(&mut self, index: &ChunkIndex) -> Result<(), SerialError> {
        if let Err(_) = self.load_chunk_from_save(index) {
            if self.chunk_loaded(index) {
                return Err(ChunkAlreadyLoaded(index.0.x, index.0.y));
            }
            println!("Addding chunk at {}", index);
            self.chunks.insert(index.clone(), Chunk::new(index, &self.gen));

            // The region this chunk was created in needs to know of the chunk
            // that was created in-game but nonexistent on disk.
            self.regions.notify_chunk_creation(index);
        }
        Ok(())
    }

    fn unload_chunk(&mut self, index: &ChunkIndex) -> Result<(), SerialError> {
        let chunk = match self.unload_chunk_internal(index) {
            Ok(c) => c,
            Err(e) => return Err(e),
        };
        let region = self.regions.get_for_chunk(index);
        region.write_chunk(chunk, index)
    }

    fn chunk_loaded(&self, index: &ChunkIndex) -> bool {
        self.chunks.contains_key(index)
    }

    fn chunk_indices(&self) -> Vec<ChunkIndex> {
        self.chunks.iter().map(|(&i, _)| i).collect()
    }

    fn update_chunks(&mut self) -> Result<(), SerialError>{
        let mut relevant: HashSet<ChunkIndex> = HashSet::new();
        let center = ChunkIndex::from_world_pos(self.observer);
        relevant.insert(center);
        let quadrant = |dx, dy, idxes: &mut HashSet<ChunkIndex>| {
            for dr in 1..UPDATE_RADIUS+1 {
                for i in 0..dr+1 {
                    let ax = center.0.x + (dr - i) * dx;
                    let ay = center.0.y + i * dy;
                    let chunk_idx = ChunkIndex::new(ax, ay);
                    idxes.insert(chunk_idx);
                }
            }
        };
        quadrant(-1,  1, &mut relevant);
        quadrant(1,   1, &mut relevant);
        quadrant(-1, -1, &mut relevant);
        quadrant(1,  -1, &mut relevant);

        for idx in relevant.iter() {
            if !self.chunk_loaded(idx) {
                println!("Loading chunk {}", idx);
                self.load_chunk(idx)?;
            }
        }

        let indices = self.chunk_indices();
        for idx in indices.iter() {
            if !relevant.contains(idx) && self.chunk_loaded(idx) {
                self.unload_chunk(idx)?;
            }
        }

        self.regions.prune_empty();

        Ok(())
    }

    fn save(mut self) -> Result<(), SerialError> {
        let indices = self.chunk_indices();
        for index in indices.iter() {
            self.unload_chunk(index)?;
        }
        Ok(())
    }
}
