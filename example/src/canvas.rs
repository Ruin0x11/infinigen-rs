use std::time::Duration;

use pancurses;

use cell::Cell;
use world::World;
use point::Point;

make_global!(WINDOW, pancurses::Window, pancurses::initscr());

pub fn get_event() -> Option<pancurses::Input> {
    instance::with(|w| w.getch())
}

pub fn print(world: &mut World) {
    instance::with_mut(|w| {
        {
            w.erase();

            let size = Point::new(w.get_max_x(), w.get_max_y());
            let center = world.observer - size/2;

            world.with_cells(center, size, |p: Point, c: &Cell| {
                                 w.mvaddch(p.y - center.y, p.x - center.x, c.to_char());
                             } );

            for dude in world.dudes() {
                let pos = dude.pos() - center;
                w.mvaddch(pos.y, pos.x, dude.appearance);
            }

            w.mvaddch(size.y/2, size.x/2, '@');
        }
        w.refresh()
    });
}

pub fn endwin() {
    pancurses::endwin();
}
