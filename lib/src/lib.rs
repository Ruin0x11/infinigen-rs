#![feature(associated_consts)]
extern crate bincode;
// extern crate flate2;
extern crate serde;
#[macro_use] extern crate serde_derive;

mod region;

mod traits;
mod managed_region;

pub use self::traits::*;
pub use self::managed_region::*;
pub use self::region::*;
