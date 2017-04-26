#![feature(associated_consts)]
extern crate infinigen;
extern crate noise;
extern crate pancurses;
extern crate rand;
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

use cell::CellKind;
use direction::Direction;
use world::World;

fn main() {
    go();
    canvas::endwin();
}

fn go() {
    let mut world = World::new_empty();

    canvas::show_splash();

    loop {
        world.update_chunks().unwrap();
        canvas::print(&mut world);

        let event = canvas::get_event().unwrap();
        match event {
            Input::Character('q') => { world.save().unwrap(); return; },
            Input::KeyUp |
            Input::Character('k') => { try_step(&mut world, Direction::N) },
            Input::KeyDown |
            Input::Character('j') => { try_step(&mut world, Direction::S) },
            Input::KeyLeft |
            Input::Character('h') => { try_step(&mut world, Direction::W) },
            Input::KeyRight |
            Input::Character('l') => { try_step(&mut world, Direction::E) },
            Input::Character('t') => { try_step(&mut world, Direction::NW) },
            Input::Character('y') => { try_step(&mut world, Direction::NE) },
            Input::Character('b') => { try_step(&mut world, Direction::SW) },
            Input::Character('n') => { try_step(&mut world, Direction::SE) },
            _                     => (),
        }

        world.step_dudes();
    }
}

fn try_step(world: &mut World, dir: Direction) {
    let new_pos = world.observer + dir;
    if world.can_walk(&new_pos) {
        world.observer = new_pos;
    } else if !world.cell(&new_pos).map_or(false, |c| c.can_walk()) {
        world.cell_mut(&new_pos).map(|c| c.kind = CellKind::Floor);
    }
}
