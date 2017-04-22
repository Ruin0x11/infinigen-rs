extern crate bincode;
extern crate caca;
extern crate serde;
#[macro_use] extern crate serde_derive;

#[macro_use] mod macros;
mod canvas;
mod cell;
mod chunk;
mod dude;
mod point;
mod serial_chunk;
mod world;

use point::Point;
use world::World;
use caca::Event;

fn pause() {
 loop {
        let event = canvas::get_event().unwrap();
        match event {
            Event::KeyPress(key) => match key {
                caca::Key::Escape => return,
                _           => break,
            },
            _ => (),
        }
    }
}

fn main() {
    let mut world = World::new(Point::new(128, 128));
    for i in 0..24 {
        world.place_dude(Point::new(i, 0));
    }
    canvas::print(&mut world);
    pause();

    world.save().unwrap();
    let mut world = World::load().unwrap();

    canvas::print(&mut world);
    pause();
}
