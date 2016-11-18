use rustwlc::{Point, ResizeEdge, WlcView};

#[derive(Clone, Copy, Debug)]
pub struct Action {
    pub view: WlcView,
    pub grab: Point,
    pub edges: ResizeEdge
}

#[derive(Clone, Copy, Debug)]
pub enum ActionErr {
    /// Tried to start an action, but an action was already in progress
    #[allow(dead_code)]
    ActionInProgress,
    /// There is not already an action in progress, but was expected to be.
    ActionNotInProgress,
    /// An action is in progress, but the lock has already been captured.
    ///
    /// NOTE that this is different from ActionInProgress. That should only
    /// be thrown when attempting to START an action, but there is already one
    /// in progress.
    ///
    /// This should be used when we WANT an action to be in progress, but we can't
    /// acquire the lock for some reason
    ActionLocked
}
