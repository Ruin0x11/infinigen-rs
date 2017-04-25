use canvas::Color;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CellKind {
    Wall,
    Floor,
    Tree,
    Nothing,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Cell {
    pub color: Color,
    pub kind: CellKind,
}

impl Cell {
    pub fn new(kind: CellKind, color: Color) -> Self{
        Cell {
            color: color,
            kind: kind
        }
    }

    pub fn to_char(&self) -> char {
        match self.kind {
            CellKind::Wall => '#',
            CellKind::Floor => '.',
            CellKind::Tree => '%',
            CellKind::Nothing => ' ',
        }
    }

    pub fn can_walk(&self) -> bool {
        match self.kind {
            CellKind::Floor => true,
            _               => false,
        }
    }
}
