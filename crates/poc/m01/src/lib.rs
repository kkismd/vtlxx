//! M01 bootstrap proof-of-concept crate.

#[allow(dead_code)] // #92 prepares the crate-private path for later source processing.
mod builder;
mod cell;
mod definition;
mod dictionary;
mod machine;
mod primitive;

pub use cell::Cell;
pub use definition::DefinitionId;
pub use machine::{ExecutionError, Machine};
