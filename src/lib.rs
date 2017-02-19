//! A crate to parse [Acorn DFS](https://en.wikipedia.org/wiki/Disc_Filing_System) disc images. Currently, only in-memory reading
//! of DFS discs is supported.

#![crate_type = "lib"]

extern crate core;

pub mod support;
pub mod dfs;
