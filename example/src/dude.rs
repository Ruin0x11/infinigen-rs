use world::WorldPosition;

#[derive(Debug, Serialize, Deserialize)]
pub struct Dude {
    pos: WorldPosition,
    pub appearance: char,
    pub name: String,
}

impl Dude {
    pub fn new(pos: WorldPosition) -> Self {
        Dude {
            pos: pos,
            appearance: 'D',
            name: "Dood".to_string(),
        }
    }

    pub fn pos(&self) -> WorldPosition {
        self.pos.clone()
    }
}
