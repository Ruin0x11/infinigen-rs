use canvas::Color;
use world::WorldPosition;

#[derive(Debug, Serialize, Deserialize)]
pub struct Dude {
    pub pos: WorldPosition,
    pub appearance: char,
    pub color: Color,
    pub name: String,
}

impl Dude {
    pub fn new(pos: WorldPosition) -> Self {
        Dude {
            pos: pos,
            appearance: 'D',
            color: Color::rand(),
            name: "Dood".to_string(),
        }
    }

    pub fn pos(&self) -> WorldPosition {
        self.pos.clone()
    }
}
