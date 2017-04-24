#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Cell {
    Wall,
    Floor,
    Tree,
    Nothing,
}

impl Cell {
    pub fn to_char(&self) -> char {
        match *self {
            Cell::Wall => '#',
            Cell::Floor => '.',
            Cell::Tree => '%',
            Cell::Nothing => ' ',
        }
    }

    pub fn can_walk(&self) -> bool {
        match *self {
            Cell::Floor => true,
            _           => false,
        }
    }
}
