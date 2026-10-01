//! M01 bootstrap proof-of-concept crate.

mod cell;
mod definition;
mod dictionary;
mod machine;
mod primitive;

pub use cell::Cell;
pub use definition::DefinitionId;
pub use machine::{ExecutionError, Machine};
