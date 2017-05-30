use std::collections::{HashSet, hash_map, HashMap};

use noise::{Perlin, Seedable};
use infinigen::*;

use cell::Cell;
use chunk::*;
use direction::Direction;
use dude::Dude;
use point::Point;

// TODO: Is there some way of using AsRef here instead, because we don't care
// about the underlying 2D point struct?
impl Index for ChunkIndex {
    fn x(&self) -> i32 { self.0.x }
    fn y(&self) -> i32 { self.0.y }
}

/// Implementation of a region manager.
pub struct Terrain {
    pub regions: HashMap<RegionIndex, Region<ChunkIndex>>,
}

impl Terrain {
    pub fn new() -> Self {
        Terrain {
            regions: HashMap::new(),
        }
    }
}

fn get_filename(index: &RegionIndex) -> String {
    format!("r.{}.{}.sr", index.0, index.1)
}

impl<'a> RegionManager<'a, ChunkIndex, SerialChunk> for Terrain
    where Region<ChunkIndex>: ManagedRegion<'a, ChunkIndex, SerialChunk>{
    fn load(&mut self, index: RegionIndex) {
        let filename = get_filename(&index);

        let handle = Region::get_region_file(filename);

        let region = Region {
            handle: Box::new(handle),
            unsaved_chunks: HashSet::new(),
        };

        self.regions.insert(index.clone(), region);
    }

    fn region_indices(&self) -> Vec<RegionIndex> {
        self.regions.iter().map(|(i, _)| i).cloned().collect()
    }

    fn get(&mut self, index: &RegionIndex) -> Option<&Region<ChunkIndex>> {
        self.regions.get(index)
    }

    fn get_mut(&mut self, index: &RegionIndex) -> Option<&mut Region<ChunkIndex>> {
        self.regions.get_mut(index)
    }

    fn remove(&mut self, index: &RegionIndex) {
        self.regions.remove(index);
    }

    fn region_loaded(&self, index: &RegionIndex) -> bool {
        self.regions.contains_key(index)
    }
}


pub type WorldPosition = Point;

impl WorldPosition {
    pub fn from_chunk_index(index: ChunkIndex) -> Point {
        Point::new(index.0.x * CHUNK_WIDTH, index.0.y * CHUNK_WIDTH)
    }
}

pub struct World {
    regions: Terrain,
    chunks: HashMap<ChunkIndex, Chunk>,
    dudes: HashMap<WorldPosition, Dude>,
    pub observer: WorldPosition,

    gen: Perlin,
}

impl World {
    pub fn new_empty() -> Self {
        World {
            regions: Terrain::new(),
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

    pub fn cell(&self, world_pos: &WorldPosition) -> Option<&Cell> {
        let chunk_pos = ChunkPosition::from_world(world_pos);
        let chunk_opt = self.chunk_from_world_pos(*world_pos);
        match chunk_opt {
            Some(chunk) => {
                Some(chunk.cell(chunk_pos))
            },
            None => None,
        }
    }

    pub fn cell_mut(&mut self, world_pos: &WorldPosition) -> Option<&mut Cell> {
        let chunk_pos = ChunkPosition::from_world(world_pos);
        let chunk_opt = self.chunk_mut_from_world_pos(*world_pos);
        match chunk_opt {
            Some(chunk) => {
                Some(chunk.cell_mut(chunk_pos))
            }
            None => None,
        }
    }

    pub fn can_walk(&self, pos: &WorldPosition) -> bool {
        let cell_walkable = self.cell(pos).map_or(false, |c| c.can_walk());
        let no_dude = self.dudes.get(pos).is_none();
        let no_player = self.observer != *pos;
        cell_walkable && no_dude && no_player
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
                            let cell_world_pos = Chunk::world_position_at(&chunk_index, &chunk_pos);
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
            let cell_world_pos = Chunk::world_position_at(chunk_index, &chunk_pos);
            if let Some(dude) = self.dudes.remove(&cell_world_pos) {
                dudes.insert(cell_world_pos, dude);
            }
        }
        dudes
    }

    pub fn step_dudes(&mut self) {
        // Not using id-based entities is painful.
        let mut actions: Vec<(WorldPosition, WorldPosition)> = Vec::new();
        for pos in self.dudes.keys() {
            let dir = Direction::choose8();
            let new_pos = *pos + dir;
            actions.push((pos.clone(), new_pos));
        }

        for (pos, new_pos) in actions {
            if self.can_walk(&new_pos) {
                let mut dude = self.dudes.remove(&pos).unwrap();
                dude.pos = new_pos.clone();
                self.dudes.insert(new_pos, dude);
            }
        }
    }
}

const UPDATE_RADIUS: i32 = 2;

impl<'a> ChunkedTerrain<'a, ChunkIndex, SerialChunk, Terrain> for World {
    fn regions_mut(&mut self) -> &mut Terrain {
        &mut self.regions
    }

    fn chunk_loaded(&self, index: &ChunkIndex) -> bool {
        self.chunks.contains_key(index)
    }

    fn chunk_indices(&self) -> Vec<ChunkIndex> {
        self.chunks.iter().map(|(&i, _)| i).collect()
    }

    fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

impl<'a> ChunkedWorld<'a, ChunkIndex, SerialChunk, Terrain, World> for World
    where Terrain: RegionManager<'a, ChunkIndex, SerialChunk> {
    fn terrain(&self) -> &World { self }
    fn terrain_mut(&mut self) -> &mut World { self }

    fn load_chunk_internal(&mut self, chunk: SerialChunk, index: &ChunkIndex) -> Result<(), SerialError> {
        for (pos, dude) in chunk.dudes.into_iter() {
            // println!("dude!");
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
        // println!("Unloading chunk at {}", index);
        let serial = SerialChunk {
            chunk: chunk,
            dudes: dudes,
        };
        Ok(serial)
    }


    fn generate_chunk(&mut self, index: &ChunkIndex) -> SerialResult<()> {
        self.chunks.insert(index.clone(), Chunk::new(index, &self.gen));

        for i in 4..8 {
            for j in 4..8 {
                let chunk_pos = ChunkPosition::from(Point::new(i, j));
                let cell_pos = Chunk::world_position_at(&index, &chunk_pos);
                if self.can_walk(&cell_pos) {
                    self.place_dude(cell_pos);
                }
            }
        }

        Ok(())
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

    fn save(&mut self) -> Result<(), SerialError> {
        let indices = self.chunk_indices();
        for index in indices.iter() {
            self.unload_chunk(index)?;
        }
        Ok(())
    }
}
