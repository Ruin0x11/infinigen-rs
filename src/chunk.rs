use std::fmt;

use cell::Cell;
use point::Point;

pub const CHUNK_WIDTH: i32 = 8;
pub const CHUNK_SIZE: i32 = CHUNK_WIDTH * CHUNK_WIDTH;

pub type ChunkPosition = Point;

#[derive(Debug, Serialize, Deserialize)]
pub struct Chunk {
    cells: Vec<Cell>,
}

impl Chunk {
    pub fn new(cell: Cell) -> Self {
        let mut cells = Vec::new();

        for _ in 0..(CHUNK_SIZE * CHUNK_SIZE) {
            cells.push(cell.clone());
        }

        Chunk {
            cells: cells
        }
    }

    /// Converts a regular Point into a ChunkPosition.
    /// The Point must be within the size of the Chunk.
    pub fn chunk_point(&self, pos: Point) -> ChunkPosition {
        assert!(pos.x >= 0);
        assert!(pos.y >= 0);
        assert!(pos.x < CHUNK_SIZE);
        assert!(pos.y < CHUNK_SIZE);
        ChunkPosition::new(pos.x, pos.y)
    }

    fn index(&self, pos: ChunkPosition) -> usize {
        (pos.y * CHUNK_SIZE + pos.x) as usize
    }

    /// Gets an immutable cell reference relative to within this Chunk.
    pub fn cell(&self, pos: ChunkPosition) -> &Cell {
        let index = self.index(pos.into());
        &self.cells[index]
    }

    /// Gets an mutable cell reference relative to within this Chunk.
    pub fn cell_mut(&mut self, pos: ChunkPosition) -> &mut Cell {
        let index = self.index(pos.into());
        &mut self.cells[index]
    }

    /// Calculates the position in the world the point in the chunk represents.
    pub fn world_position(&self, index: &ChunkIndex, pos: &ChunkPosition) -> Point {
        Point::new(pos.x + index.0.x * CHUNK_SIZE, pos.y + index.0.y * CHUNK_SIZE)
    }

    pub fn iter(&self) -> Cells {
        Cells {
            index: 0,
            width: CHUNK_SIZE,
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
        let level_position = ChunkPosition::new(x, y);
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
                // [-1, -chunk_size] = -1
                ((i + 1) / CHUNK_SIZE) - 1
            } else {
                // [0, chunk_size-1] = 0
                i / CHUNK_SIZE
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

