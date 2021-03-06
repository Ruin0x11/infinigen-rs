use std::collections::HashMap;
use std::fmt;

use infinigen::ManagedChunk;
use noise::{NoiseModule, Perlin};

use canvas::Color;
use cell::*;
use dude::Dude;
use point::Point;
use world::WorldPosition;

pub const CHUNK_WIDTH: i32 = 32;

#[derive(Debug, Clone)]
pub struct ChunkPosition(pub Point);

impl From<Point> for ChunkPosition {
    fn from(pos: Point) -> ChunkPosition {
        assert!(pos.x >= 0);
        assert!(pos.y >= 0);
        assert!(pos.x < CHUNK_WIDTH);
        assert!(pos.y < CHUNK_WIDTH);
        ChunkPosition(pos)
    }
}

impl ChunkPosition {
    pub fn from_world(pos: &WorldPosition) -> ChunkPosition {
        let conv = |i: i32| {
            let i_new = i % CHUNK_WIDTH;
            if i_new < 0 {
                CHUNK_WIDTH + i_new
            } else {
                i_new
            }
        };
        ChunkPosition(Point::new(conv(pos.x), conv(pos.y)))
    }
}

impl fmt::Display for ChunkPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chunk {
    cells: Vec<Cell>,
}

const COS_THETA: f32 = 0.99854;
const SIN_THETA: f32 = 0.05408;
const NOISE_SCALE: f32 = 0.05;
const THRESHOLD: f32 = 0.30;

impl Chunk {
    pub fn new(index: &ChunkIndex, gen: &Perlin) -> Self {
        let mut cells = Vec::new();
        let center = WorldPosition::from_chunk_index(*index);

        let fg_color = Color::rand();
        let bg_color = Color::rand();

        for j in 0..(CHUNK_WIDTH) {
            for i in 0..(CHUNK_WIDTH) {
                let ax = (center.x + i) as f32;
                let ay = (center.y + j) as f32;
                let az = 0.2333333333;

                // Perlin doesn't work on integer values, so rotate slightly.
                let conv = |a: f32, b| NOISE_SCALE * (a * COS_THETA + b * SIN_THETA);
                let res = gen.get([conv(ay, -ax), conv(ax, ay), az]);

                if res > THRESHOLD {
                    cells.push(Cell::new(CellKind::Tree, bg_color));
                } else {
                    cells.push(Cell::new(CellKind::Floor, bg_color));
                }

                let res = gen.get([conv(ay, -ax), conv(ax, ay), az + 4.555555555]);
                let index = Chunk::cell_index(ChunkPosition(Point::new(i, j)));
                if res > THRESHOLD {
                    cells[index].color = fg_color;
                }
            }
        }

        Chunk {
            cells: cells
        }
    }

    fn cell_index(pos: ChunkPosition) -> usize {
        (pos.0.y * CHUNK_WIDTH + pos.0.x) as usize
    }

    /// Gets an immutable cell reference relative to within this Chunk.
    pub fn cell(&self, pos: ChunkPosition) -> &Cell {
        let index = Chunk::cell_index(pos.into());
        &self.cells[index]
    }

    /// Gets an mutable cell reference relative to within this Chunk.
    pub fn cell_mut(&mut self, pos: ChunkPosition) -> &mut Cell {
        let index = Chunk::cell_index(pos.into());
        &mut self.cells[index]
    }

    /// Calculates the position in the world the point in the chunk represents.
    pub fn world_position_at(index: &ChunkIndex, pos: &ChunkPosition) -> Point {
        Point::new(pos.0.x + index.0.x * CHUNK_WIDTH, pos.0.y + index.0.y * CHUNK_WIDTH)
    }

    pub fn iter(&self) -> Cells {
        Cells {
            index: 0,
            width: CHUNK_WIDTH,
            inner: self.cells.iter(),
        }
    }
}

pub struct Cells<'a> {
    index: i32,
    width: i32,
    inner: ::std::slice::Iter<'a, Cell>,
}

impl<'a> Iterator for Cells<'a> {
    type Item = (ChunkPosition, &'a Cell);

    fn next(&mut self) -> Option<(ChunkPosition, &'a Cell)> {
        let x = self.index % self.width;
        let y = self.index / self.width;
        let level_position = ChunkPosition(Point::new(x, y));
        self.index += 1;
        match self.inner.next() {
            Some(cell) => {
                Some((level_position, cell))
            }
            None => None,
        }
    }
}

// Because a world position and chunk index are different quantities, newtype to
// enforce correct usage
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ChunkIndex(pub Point);

impl ChunkIndex {
    pub fn new(x: i32, y: i32) -> Self {
        ChunkIndex(Point::new(x, y))
    }

    pub fn from_world_pos(pos: Point) -> ChunkIndex {
        let conv = |i: i32| {
            if i < 0 {
                // [-1, -chunk_width] = -1
                ((i + 1) / CHUNK_WIDTH) - 1
            } else {
                // [0, chunk_width-1] = 0
                i / CHUNK_WIDTH
            }
        };

        ChunkIndex::new(conv(pos.x), conv(pos.y))
    }

}

impl fmt::Display for ChunkIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerialChunk {
    pub chunk: Chunk,
    pub dudes: HashMap<WorldPosition, Dude>,
}

impl ManagedChunk for SerialChunk {
    const SECTOR_SIZE: usize = 4096;

    const REGION_WIDTH: i32 = 32;
}
