extern crate infinigen;
extern crate noise;
extern crate pancurses;
extern crate serde;
#[macro_use] extern crate serde_derive;

#[macro_use] mod macros;
mod canvas;
mod cell;
mod chunk;
mod direction;
mod dude;
mod point;
mod world;

use infinigen::Chunked;
use pancurses::Input;

use cell::Cell;
use direction::Direction;
use point::Point;
use world::World;

fn main() {
    go();
    canvas::endwin();
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

        let event = canvas::get_event().unwrap();
        match event {
            Input::Character('q') => { world.save().unwrap(); return; },
            Input::Character('k') => { try_step(&mut world, Direction::N) },
            Input::Character('j') => { try_step(&mut world, Direction::S) },
            Input::Character('h') => { try_step(&mut world, Direction::W) },
            Input::Character('l') => { try_step(&mut world, Direction::E) },
            _                     => (),
        }
    }
}

fn try_step(world: &mut World, dir: Direction) {
    let new_pos = world.observer + dir;
    let can_walk = world.cell_mut(&new_pos).map_or(false, |c| c.can_walk());
    if !can_walk {
        world.cell_mut(&new_pos).map(|c| *c = Cell::Floor);
    } else {
        world.observer = new_pos;
    }
}
