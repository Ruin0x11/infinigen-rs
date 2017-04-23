use std::collections::{HashSet, hash_map, HashMap};

use cell::Cell;
use chunk::*;
use dude::Dude;
use point::Point;
use region::*;
use serial_chunk::*;

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
}

impl World {
    pub fn new(size: Point) -> Self {
        let mut world = World::new_empty();
        let chunks = World::generate_chunks(size.x, size.y);
        world.chunks = chunks;
        world
    }

    pub fn new_empty() -> Self {
        World {
            regions: RegionManager::new(),
            chunks: HashMap::new(),
            dudes: HashMap::new(),
            observer: WorldPosition::new(0, 0),
        }
    }

    fn generate_chunks(width: i32, height: i32) -> HashMap<ChunkIndex, Chunk> {
        assert!(width > 0);
        assert!(height > 0);

        let mut chunks = HashMap::new();

        let ceiling = |q: i32, d: i32| (q + d - 1) / d;
        let columns = ceiling(width, CHUNK_WIDTH);
        let rows = ceiling(height, CHUNK_WIDTH);

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
    pub fn load_chunk(&mut self, index: &ChunkIndex) -> Result<(), SerialError> {
        let region = self.regions.get_for_chunk(index);
        let chunk = match region.read_chunk(index) {
            Ok(c) => c,
            Err(e) => return Err(e),
        };
        println!("{:?}", chunk.dudes);

        println!("Loading chunk at {}", index);
        for (pos, dude) in chunk.dudes.into_iter() {
            println!("dude!");
            self.dudes.insert(pos, dude);
        }
        self.chunks.insert(index.clone(), chunk.chunk);

        Ok(())
    }

    pub fn load_or_gen_chunk(&mut self, index: &ChunkIndex) -> Result<(), SerialError> {
        if let Err(_) = self.load_chunk(index) {
            if self.chunk_loaded(index) {
                return Err(ChunkAlreadyLoaded(index.clone()));
            }
            println!("Addding chunk at {}", index);
            self.chunks.insert(index.clone(), Chunk::new(Cell::Floor));
        }
        Ok(())
    }

    pub fn save_and_unload_chunk(&mut self, index: &ChunkIndex) -> Result<(), SerialError> {
        let chunk = match self.unload_chunk(index) {
            Ok(c) => c,
            Err(e) => return Err(e),
        };
        let region = self.regions.get_for_chunk(index);
        region.write_chunk(chunk, index)
    }

    fn unload_chunk(&mut self, index: &ChunkIndex) -> Result<SerialChunk, SerialError> {
        let chunk = match self.chunks.remove(&index) {
            Some(c) => c,
            None => return Err(NoChunkInWorld(index.clone())),
        };
        let dudes = self.remove_dudes_in_chunk(&index, &chunk);
        println!("Unloading chunk at {}", index);
        let serial = SerialChunk {
            chunk: chunk,
            dudes: dudes,
        };
        Ok(serial)
    }

    fn chunk_loaded(&self, index: &ChunkIndex) -> bool {
        self.chunks.contains_key(index)
    }

    pub fn chunk_indices(&self) -> Vec<ChunkIndex> {
        self.chunks.iter().map(|(&i, _)| i).collect()
    }

    pub fn save(mut self) -> Result<(), SerialError> {
        let indices = self.chunk_indices();
        for index in indices.iter() {
            self.save_and_unload_chunk(index)?;
        }
        Ok(())
    }

    pub fn load() -> Result<Self, SerialError> {
        let index = ChunkIndex::new(0, 0);
        let mut world = World::new_empty();
        world.load_chunk(&index);
        Ok(world)
    }
}

const UPDATE_RADIUS: i32 = 3;

impl World {
    pub fn update_chunks(&mut self) -> Result<(), SerialError>{
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

        let mut d = HashSet::new();

        for idx in relevant.iter() {
            if !self.chunk_loaded(idx) {
                println!("Loading chunk {}", idx);
                self.load_or_gen_chunk(idx)?;
                d.insert(idx.clone());
            }
        }

        let indices = self.chunk_indices();
        for idx in indices.iter() {
            if !relevant.contains(idx) && self.chunk_loaded(idx) {
                self.save_and_unload_chunk(idx)?;
            }
        }

        for i in d.iter() {
            println!("{}", i);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saveload() {
        let mut world = World::new(Point::new(128, 128));
        let index = ChunkIndex::new(0, 0);

        world.place_dude(Point::new(0, 0));
        let count_before = world.dudes.len();

        world.save_and_unload_chunk(&index).unwrap();
        world.load_chunk(&index).unwrap();

        assert_eq!(count_before, world.dudes.len());
    }
}
