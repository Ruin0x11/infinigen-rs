use pancurses;
use pancurses::*;
use rand;

use cell::Cell;
use world::World;
use point::Point;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Color {
    Red,
    Blue,
    Green,
    Cyan,
    Magenta,
    Yellow,
    White
}

impl Color {
    pub fn rand() -> Color {
        let len = 7;    //ROYGBIV
        //use Color::*;
        match rand::random::<u8>() % len {
            0 => Color::Red,
            1 => Color::Blue,
            2 => Color::Green,
            3 => Color::Cyan,
            4 => Color::Magenta,
            5 => Color::Yellow,
            _ => Color::White,
        }
    }
    pub fn to_pancurses(&self) -> ColorPair {
        pancurses::ColorPair((*self as u8))
    }
}

const COLOR_TABLE: [i16; 8] = [COLOR_RED,
                               COLOR_BLUE,
                               COLOR_GREEN,
                               COLOR_CYAN,
                               COLOR_RED,
                               COLOR_MAGENTA,
                               COLOR_YELLOW,
                               COLOR_WHITE];

make_global!(WINDOW, pancurses::Window, pancurses::initscr());

pub fn get_event() -> Option<pancurses::Input> {
    instance::with(|w| w.getch())
}

pub fn init() {
    pancurses::start_color();
    pancurses::curs_set(0);

    for (i, color) in COLOR_TABLE.into_iter().enumerate() {
        pancurses::init_pair(i as i16, *color, COLOR_BLACK);
    }
}

pub fn show_splash() {
    instance::with_mut(|w| {
        init();
        w.erase();
        w.mvaddstr(0, 0, "move: hjkltybn");
        w.mvaddstr(1, 0, "quit: q");
        w.mvaddstr(2, 0, "run into walls to destroy them.");
        w.mvaddstr(3, 0, "autosave on quit.");
        w.refresh();
        w.getch();
    })
}

pub fn print(world: &mut World) {
    instance::with_mut(|w| {
        w.erase();

        let size = Point::new(w.get_max_x(), w.get_max_y());
        let center = world.observer - size/2;

        world.with_cells(center, size, |p: Point, c: &Cell| {
            w.attrset(c.color.to_pancurses());
            w.mvaddch(p.y - center.y, p.x - center.x, c.to_char());
        } );

        for dude in world.dudes() {
            let pos = dude.pos() - center;
            w.attrset(dude.color.to_pancurses());
            w.mvaddch(pos.y, pos.x, dude.appearance);
        }
        w.attrset(Color::White.to_pancurses());
        w.mvaddch(size.y/2, size.x/2, '@');

        w.refresh()
    });
}

pub fn endwin() {
    pancurses::endwin();
}
