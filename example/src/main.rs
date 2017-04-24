extern crate infinigen;
extern crate caca;
extern crate noise;
extern crate serde;
#[macro_use] extern crate serde_derive;

#[macro_use] mod macros;
mod world;
mod dude;
mod canvas;
mod cell;
mod chunk;
mod point;

use point::Point;
use world::World;
use caca::Event;
use infinigen::Chunked;

fn main() {
    go();
}

fn go() {
    // let mut world = World::new(Point::new(128, 128));
    let mut world = World::new_empty();
    for i in 0..24 {
        world.place_dude(Point::new(i, 0));
    }

    loop {
        world.update_chunks().unwrap();
        canvas::print(&mut world);

        loop {
            let event = canvas::get_event().unwrap();
            match event {
                Event::KeyPress(key)  => match key {
                    caca::Key::Escape => {world.save().unwrap(); return;},
                    caca::Key::Up     => {world.observer.y -= 1; break;},
                    caca::Key::Down   => {world.observer.y += 1; break;},
                    caca::Key::Left   => {world.observer.x -= 1; break;},
                    caca::Key::Right  => {world.observer.x += 1; break;},
                    _                 => break,
                },
                _ => (),
            }
        }
    }
}
