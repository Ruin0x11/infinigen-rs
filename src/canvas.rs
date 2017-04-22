use std::time::Duration;

use caca::{self, AnsiColor};

use cell::Cell;
use world::World;
use point::Point;

make_global!(DISPLAY, caca::Display, make_display(80, 40));

pub fn set_size(w: i32, h: i32) {
    instance::with_mut(|d| d.canvas().set_size(w, h));
}

pub fn get_event() -> Option<caca::Event> {
    instance::with(|d| d.poll_event(caca::EVENT_ANY.bits()))
}

pub fn print(world: &mut World) {
    instance::with_mut(|d| {
        {
            let mut canvas = d.canvas();
            canvas.clear();
            canvas.set_color_ansi(&AnsiColor::LightGray, &AnsiColor::Black);

            let center = world.observer;

            let size = Point::new(80, 40);
            world.with_cells(center, size, |p: Point, c: &Cell| {
                                 canvas.put_char(p.x - center.x, p.y - center.y, c.to_char());
                             } );

            for dude in world.dudes() {
                let pos = center + dude.pos();
                canvas.put_char(pos.x, pos.y, dude.appearance);
            }
        }
        d.set_display_time(Duration::new(30, 10000));
        d.refresh();
    });
}

fn make_display(w: i32, h: i32) -> caca::Display {
    let canvas = caca::Canvas::new(w, h).unwrap();
    caca::Display::new(caca::InitOptions{ canvas: Some(&canvas), .. caca::InitOptions::default()}).unwrap()
}

pub fn clear() {
    instance::with_mut(|d| d.canvas().clear());
}

pub fn put(x: i32, y: i32, ch: char) {
    instance::with_mut(|d| {
        let mut canvas = d.canvas();
        canvas.set_color_ansi(&AnsiColor::White, &AnsiColor::Black);
        canvas.put_char(x, y, ch);
    })
}
