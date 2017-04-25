use super::super::commands::CommandResult;
use super::super::{LayoutTree, TreeError};
use super::super::core::Direction;
use super::super::core::container::{Container, ContainerType, Layout};

use petgraph::graph::NodeIndex;
use rustwlc::WlcView;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FocusError {
    /// Reached a container where we can keep climbing the tree no longer.
    ///
    /// Usually this is a workspace, because it doesn't make sense to move a
    /// container out of a workspace
    ReachedLimit(NodeIndex),
    /// Tried to focus on a container that was not a view.
    NotAView(Uuid),
    /// Tried to focus on a container (first one),
    /// but that container was superseded by a fullscreen container (second one)
    BlockedByFullscreen(Uuid, Uuid)
}

impl LayoutTree {
    /// Focuses on the container by the uuid, if it points to a View.
    /// Otherwise, an error is returned.
    pub fn focus_on(&mut self, uuid: Uuid) -> CommandResult {
        if let Some(fullscreen_id) = try!(self.in_fullscreen_workspace(uuid)) {
            if fullscreen_id != uuid {
                return Err(TreeError::Focus(FocusError::BlockedByFullscreen(uuid, fullscreen_id)))
            }
        }
        let node_ix = self.tree.lookup_id(uuid)
            .ok_or(TreeError::NodeNotFound(uuid))?;
        match self.tree[node_ix] {
            Container::View { handle, .. } => {
                handle.focus();
                self.active_container = Some(node_ix);
            },
            _ => return Err(TreeError::Focus(FocusError::NotAView(uuid)))
        }
        if !self.tree[node_ix].floating() {
            self.tree.set_ancestor_paths_active(node_ix);
        }
        Ok(())
    }
    /// Focus on the container relative to the active container.
    ///
    /// If Horizontal, left and right will move within siblings.
    /// If Vertical, up and down will move within siblings.
    /// Other wise, it moves to the next sibling of the parent container.
    ///
    /// If the edge of the children is hit, it does not wrap around,
    /// but moves between ancestor siblings.
    pub fn move_focus(&mut self, direction: Direction) -> CommandResult {
        if let Some(prev_active_ix) = self.active_container {
            let active_id = self.tree[prev_active_ix].get_id();
            if let Some(fullscreen_id) = try!(self.in_fullscreen_workspace(active_id)) {
                return Err(TreeError::Focus(
                    FocusError::BlockedByFullscreen(active_id, fullscreen_id)))
            }
            let new_active_ix = self.move_focus_recurse(prev_active_ix, direction)
                .unwrap_or(prev_active_ix);
            try!(self.set_active_node(new_active_ix));
            match self.tree[self.active_container.unwrap()] {
                Container::View { ref handle, .. } => handle.focus(),
                _ => warn!("move_focus returned a non-view, cannot focus")
            }
        } else {
            return Err(TreeError::NoActiveContainer)
        }
        self.validate();
        Ok(())
    }

    fn move_focus_recurse(&mut self, node_ix: NodeIndex, direction: Direction)
                          -> Result<NodeIndex, TreeError> {
        match self.tree[node_ix].get_type() {
            ContainerType::View | ContainerType::Container => { /* continue */ },
            _ => return Err(TreeError::UuidWrongType(self.tree[node_ix].get_id(),
                                                     vec!(ContainerType::View, ContainerType::Container)))
        }
        let parent_ix = self.tree.parent_of(node_ix)
            .expect("Active ix had no parent");
        match self.tree[parent_ix] {
            Container::Container { layout, .. } => {
                match (layout, direction) {
                    (Layout::Scroll, Direction::Left) |
                    (Layout::Scroll, Direction::Right) |
                    (Layout::Horizontal, Direction::Left) |
                    (Layout::Horizontal, Direction::Right) |
                    (Layout::Vertical, Direction::Up) |
                    (Layout::Vertical, Direction::Down) => {
                        let siblings = self.tree.children_of(parent_ix);
                        let cur_index = siblings.iter().position(|node| {
                            *node == node_ix
                        }).expect("Could not find self in parent");
                        let maybe_new_index = match direction {
                            Direction::Right | Direction::Down => {
                                cur_index.checked_add(1)
                            }
                            Direction::Left  | Direction::Up => {
                                cur_index.checked_sub(1)
                            }
                        };
                        if maybe_new_index.is_some() &&
                            maybe_new_index.unwrap() < siblings.len() {
                                // There is a sibling to move to.
                                let new_index = maybe_new_index.unwrap();
                                let new_active_ix = siblings[new_index];
                                match self.tree[new_active_ix].get_type() {
                                    ContainerType::Container => {
                                        let path_ix = self.tree.follow_path(new_active_ix);
                                        // If the path wasn't complete, find the first view and focus on that
                                        let node_ix = try!(self.tree.descendant_of_type(path_ix, ContainerType::View)
                                                           .map_err(|err| TreeError::PetGraph(err)));
                                        let parent_ix = try!(self.tree.parent_of(node_ix)
                                                             .map_err(|err| TreeError::PetGraph(err)));
                                        match self.tree[node_ix].get_type() {
                                            ContainerType::View | ContainerType::Container => {},
                                            _ => panic!("Following path did not lead to a container or a view!")
                                        }
                                        trace!("Moving to different view {:?} in container {:?}",
                                                self.tree[node_ix], self.tree[parent_ix]);
                                        return Ok(node_ix);
                                    },
                                    ContainerType::View => {
                                        trace!("Moving to other view {:?}", self.tree[new_active_ix]);
                                        return Ok(new_active_ix)
                                    },
                                    _ => unreachable!()
                                };
                            }
                    },
                    _ => { /* We are moving out of siblings, recurse */ }
                }
            }
            Container::Workspace { .. } => {
                return Err(TreeError::Focus(FocusError::ReachedLimit(parent_ix)));
            }
            _ => unreachable!()
        }
        let parent_ix = self.tree.parent_of(node_ix)
            .expect("Node had no parent");
        return self.move_focus_recurse(parent_ix, direction);
    }

    /// Updates the current active container to be the next container or view
    /// to focus on after the previous view/container was moved/removed.
    ///
    /// A new view will tried to be set, starting with the children of the
    /// parent node. If a view cannot be found there, it starts climbing the
    /// tree until either a view is found or the workspace is (in which case
    /// it set the active container to the root container of the workspace)
    ///
    /// If there are siblings, the chosen node is the one with the lowest
    /// active count.
    ///
    /// If there is a fullscreen container in this workspace, that is focused on next,
    /// with the active path updated accordingly.
    pub fn focus_on_next_container(&mut self, mut parent_ix: NodeIndex) {
        let last_ix = self.tree.active_path().last()
            .expect("Active path did not lead anywhere").0;
        let id = self.tree[last_ix].get_id();
        match self.in_fullscreen_workspace(id) {
            Ok(Some(fullscreen_id)) => {
                self.set_active_container(fullscreen_id)
                    .unwrap_or_else(|err| {
                        error!("Could not set {:?} to be the active container, \
                                even though it's the fullscreen container! {:?}",
                               fullscreen_id, err);
                        error!("{:#?}", self);
                        panic!("Could not set the fullscreen container to be \
                                the active container!")
                    });
                return;
            },
            _ => {}
        }
        match self.tree[last_ix] {
            Container::View { handle, .. } => {
                handle.focus();
                self.active_container = Some(last_ix);
                return
            },
            Container::Container { .. } => {
                parent_ix = last_ix;
            },
            _ => {}
        }
        while self.tree.node_type(parent_ix)
            .expect("Node not part of the tree") != ContainerType::Workspace {
                if let Some(node_ix) = self.tree.lowest_active_view(parent_ix) {
                    match self.tree[node_ix] {
                        Container::View { .. } => {
                            trace!("Active container set to view at {:?}", node_ix);
                            let id = self.tree[node_ix].get_id();
                            self.set_active_container(id)
                                .expect("Could not set active container");
                            return;
                        },
                        _ => {}
                    }
                }
                parent_ix = self.tree.ancestor_of_type(parent_ix,
                                                       ContainerType::Container)
                    .unwrap_or_else(|_| {
                        self.tree.ancestor_of_type(parent_ix, ContainerType::Workspace)
                            .expect("Container was not part of a workspace")
                    });
        }
        // If this is reached, parent is workspace
        let container_ix = self.tree.children_of(parent_ix).get(0).cloned();
        if container_ix.is_none() {
            trace!("There were no other containers to focus on, \
                    focusing on nothing in particular!");
            return;
        }
        let container_ix = container_ix.unwrap();
        let root_c_children = self.tree.grounded_children(container_ix);
        if root_c_children.len() > 0 {
            // Only searches first child of root container, can't be floating view.
            let new_active_ix = self.tree.descendant_of_type(root_c_children[0],
                                                                ContainerType::View)
                .unwrap_or(root_c_children[0]);
            let id = self.tree[new_active_ix].get_id();
            self.set_active_container(id)
                .expect("Could not set active container");
            match self.tree[new_active_ix] {
                Container::View { ref handle, .. } => {
                    debug!("Focusing on {:?}", handle);
                    handle.focus();
                },
                Container::Container { .. } => {
                    debug!("No view found, focusing on nothing in workspace {:?}", parent_ix);
                    WlcView::root().focus();
                }
                _ => unreachable!()
            };
            return;
        } else {
            let floating_children = self.tree.floating_children(container_ix);
            for child_ix in floating_children {
                if let Ok(view_ix) = self.tree.descendant_of_type(child_ix,
                                                                    ContainerType::View) {
                    match self.tree[view_ix] {
                        Container::View { handle, id, .. } => {
                            debug!("Floating view found, focusing on {:#?}", handle);
                            handle.focus();
                            self.set_active_container(id)
                                .expect("Could not set active container");
                            return;
                        },
                        _ => unreachable!()
                    };
                }
            }
        }
        trace!("Active container set to container {:?}", container_ix);
        let id = self.tree[container_ix].get_id();
        self.set_active_container(id)
            .expect("Could not set active container");

        // Update focus to new container
        self.get_active_container().map(|con| match *con {
            Container::View { ref handle, .. } => handle.focus(),
            Container::Container { .. } => WlcView::root().focus(),
            _ => panic!("Active container not view or container!")
        });
    }

    /// If the currently focused view is floating, then the non-floating at the end of
    /// the path becomes the focused view. Otherwise, the first floating view becomes
    /// the focused view.
    ///
    /// If there is no currently focused view, does nothing.
    pub fn toggle_floating_focus(&mut self) -> CommandResult {
        let active_ix = try!(self.active_container.ok_or(TreeError::NoActiveContainer));
        let id; let floating;
        {
            let container = &self.tree[active_ix];
            id = container.get_id();
            floating = container.floating();
        }
        if let Some(fullscreen_id) = try!(self.in_fullscreen_workspace(id)) {
            return Err(TreeError::Focus(FocusError::BlockedByFullscreen(id, fullscreen_id)))
        }
        if floating {
            let root_ix = self.tree.root_ix();
            let new_ix = self.tree.follow_path(root_ix);
            match self.tree[new_ix].get_type() {
                ContainerType::View | ContainerType::Container  => {
                    try!(self.set_active_node(new_ix));
                    Ok(())
                },
                type_ => {
                    error!("Path lead to the wrong container, {:#?}\n{:#?}\n{:#?}",
                           active_ix, type_, self);
                    panic!("toggle_floating_focused: bad path");
                }
            }
        } else {
            // Current view is not floating, gotta focus on floating view
            let root_c_ix = self.root_container_ix()
                .expect("No root container ancestor of active container");
            let floating_children = self.tree.floating_children(root_c_ix);
            if floating_children.len() > 0 {
                try!(self.set_active_node(floating_children[0]));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::core::tree::tests::basic_tree;
    use rustwlc::*;

    /// Tests the new algorithm, the one that i3 uses, to determine which
    /// sibling to focus on when the active one is closed.
    #[test]
    fn test_sibling_focus_algorithm() {
        let mut tree = basic_tree();
        let fake_view = WlcView::root();
        tree.switch_to_workspace("some_unique_workspace");
        let view_1 = tree.add_view(fake_view).unwrap().get_id();
        let view_2 = tree.add_view(fake_view).unwrap().get_id();
        let view_3 = tree.add_view(fake_view).unwrap().get_id();
        let view_4 = tree.add_view(fake_view).unwrap().get_id();
        let view_5 = tree.add_view(fake_view).unwrap().get_id();

        tree.focus_on(view_3).unwrap();
        tree.focus_on(view_1).unwrap();
        // should focus 1 -> 3 -> 5 -> 4 -> 2
        // because this the "stack" of viewing them.
        // NOTE that adding a view implicitly focuses and adds it to the "stack"
        let views = vec![view_1, view_3, view_5, view_4, view_2];
        for view in views {
            let active_ix = tree.tree.lookup_id(view);
            assert_eq!(tree.active_container, active_ix);
            tree.remove_active().unwrap();
        }
    }

    /// Tests that after sending a floating view to a new workspace,
    /// there are no duplicate active numbers (and we can focus on that
    /// workspace with no problem)
    ///
    /// The expected behaviour is that we focus on the floating windows that
    /// were sent over (e.g, the latest focused view from the user's perspective
    /// is reflected even when sent to a different workspace).
    #[test]
    fn test_focus_on_floating_after_sending_to_workspace() {
        let mut tree = basic_tree();
        let fake_view = WlcView::root();
        let target_workspace = "some_unique_workspace";
        let source_workspace = "a different workspace";
        tree.switch_to_workspace(target_workspace);
        let _unused_tiled_view = tree.add_view(fake_view).unwrap().get_id();

        tree.switch_to_workspace(source_workspace);
        let floating_view_1 = tree.add_floating_view(fake_view, None).unwrap().get_id();
        let floating_view_2 = tree.add_floating_view(fake_view, None).unwrap().get_id();
        tree.focus_on(floating_view_2).unwrap();
        assert_eq!(tree.tree.lookup_id(floating_view_2), tree.active_container);

        // now send the view to the workspace with the tiled window.
        tree.send_active_to_workspace(target_workspace);
        tree.switch_to_workspace(target_workspace);
        assert_eq!(tree.tree.lookup_id(floating_view_2), tree.active_container);

        // and again with the other one
        tree.switch_to_workspace(source_workspace);
        tree.send_active_to_workspace(target_workspace);
        tree.switch_to_workspace(target_workspace);
        assert_eq!(tree.tree.lookup_id(floating_view_1), tree.active_container);

    }
}
