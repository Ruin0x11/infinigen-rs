use std::collections::HashMap;

use chunk::*;
use world::*;
use dude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct SerialChunk {
    pub chunk: Chunk,
    pub dudes: HashMap<WorldPosition, Dude>,
}
