//! Host orchestration ops: docker, containers, images, and external tools.

pub mod container;
pub mod docker;
pub mod gh;
pub mod herdr;
pub mod images;
pub mod op;
pub mod transcript;

pub use taskfmt::ops::git;
pub use taskfmt::ops::{
    Captured, capture, capture_with_timeout, check, copy_tree, copy_tree_filtered,
    sorted_files_by_name, symlink, trace, write_file,
};
