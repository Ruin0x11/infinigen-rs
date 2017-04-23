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
mod region;
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

fn saveload() {
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
