use std::cmp::{PartialOrd, Ord, Ordering};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// A path through the tree, holds metadate associated with nodes
/// and their connections to other nodes.
pub struct Path {
    /// The weight used to order this `Path` with other `Paths`.
    pub weight: u32,
    /// A flag to indicate if this path has been active lately.
    // This allows the tree to 'remember' recently used paths.
    pub active: bool
}

/// Builder for `Path`, allows more fields to be added and maintain backwards
/// compatibility for `Path` construction
pub struct PathBuilder {
    path: Path
}

impl PathBuilder {
    /// Construct new builder, with the given weight.
    ///
    /// Active is set to false automatically
    pub fn new(weight: u32) -> Self {
        PathBuilder { path: Path { weight: weight, active: false } }
    }

    pub fn active(mut self, value: bool) -> Self {
        self.path.active = value;
        self
    }

    pub fn build(self) -> Path {
        self.path
    }
}

impl Path {
    /// Returns an active path with a weight of zero
    pub fn zero() -> Self {
        Path { weight: 0, active: true }
    }
}


impl Ord for Path {
    fn cmp(&self, other: &Self) -> Ordering {
        self.weight.cmp(&other.weight)
    }
}

impl PartialOrd for Path {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Deref for Path {
    type Target = u32;

    fn deref(&self) -> &u32 {
        &self.weight
    }
}

impl DerefMut for Path {
    fn deref_mut(&mut self) -> &mut u32 {
        &mut self.weight
    }
}
