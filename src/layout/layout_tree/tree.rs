//! Main module to handle the layout.
//! This is where the i3-specific code is.

use petgraph::graph::NodeIndex;

use rustwlc::{WlcView, WlcOutput, Geometry, Point};

use super::super::LayoutTree;

use super::super::container::{Container, Handle, ContainerType, Layout};

#[derive(Clone, Copy)]
pub enum Direction {
    Up,
    Down,
    Right,
    Left
}

impl LayoutTree {
    /// Drops every node in the tree, essentially invalidating it
    pub fn destroy_tree(&mut self) {
        let root_ix = self.tree.root_ix();
        let mut nodes = self.tree.all_descendants_of(&root_ix);
        nodes.sort_by(|a, b| b.cmp(a));
        for node in nodes {
            self.tree.remove(node);
        }
        self.unset_active_container();
    }

    /// Sets the active container by finding the node with the WlcView
    pub fn set_active_container(&mut self, handle: WlcView) {
        info!("Active container was: {:?}", self.active_container);
        if let Some(node_ix) = self.tree.descendant_with_handle(self.tree.root_ix(), &handle) {
            self.active_container = Some(node_ix);
            handle.focus();
        } else {
            warn!("Could not find handle {:?}", handle);
        }
        info!("Active container is now: {:?}", self.active_container);
    }

    /// Unsets the active container. This should be used when focusing on
    /// a view that is not a part of the tree.
    pub fn unset_active_container(&mut self) {
        self.active_container = None;
    }

    /// Gets the currently active container.
    pub fn get_active_container(&self) -> Option<&Container> {
        self.active_container.and_then(|ix| self.tree.get(ix))
    }

    #[allow(dead_code)]
    /// Gets the currently active container.
    fn get_active_container_mut(&mut self) -> Option<&mut Container> {
        self.active_container.and_then(move |ix| self.tree.get_mut(ix))
    }

    #[allow(dead_code)]
    /// Gets the parent of the active container
    fn get_active_parent(&self) -> Option<&Container> {
        if let Some(container_ix) = self.active_container {
            if let Some(parent_ix) = self.tree.parent_of(container_ix) {
                return Some(&self.tree[parent_ix]);
            }
        }
        return None
    }

    #[allow(dead_code)]
    /// Gets the active output. This contains the WlcOutput
    fn get_active_output(&self) -> Option<&Container> {
        self.active_of(ContainerType::Output)
    }

    #[allow(dead_code)]
    /// Gets the active output. This contains the WlcOutput
    fn get_active_output_mut(&mut self) -> Option<&mut Container> {
        self.active_of_mut(ContainerType::Output)
    }

    #[allow(dead_code)]
    /// Gets the workspace the active container is located on
    fn get_active_workspace(&self) -> Option<&Container> {
        self.active_of(ContainerType::Workspace)
    }

    #[allow(dead_code)]
    /// Gets the workspace the active container is located on
    fn get_active_workspace_mut(&mut self) -> Option<&mut Container> {
        self.active_of_mut(ContainerType::Workspace)
    }

    /// Gets the index of the currently active container with the given type.
    /// Starts at the active container, moves up until either a container with
    /// that type is found or the root node is hit
    pub fn active_ix_of(&self, ctype: ContainerType) -> Option<NodeIndex> {
        if let Some(ix) = self.active_container {
            if self.tree[ix].get_type() == ctype {
                return Some(ix)
            }
            return self.tree.ancestor_of_type(ix, ctype)
        }
        return None
    }

    /// Gets the active container if there is a currently focused container
    fn active_of(&self, ctype: ContainerType) -> Option<&Container> {
        self.active_ix_of(ctype).and_then(|ix| self.tree.get(ix))
    }

    /// Gets the active container mutably, if there is a currently focused container
    fn active_of_mut(&mut self, ctype: ContainerType) -> Option<&mut Container> {
        self.active_ix_of(ctype).and_then(move |ix| self.tree.get_mut(ix))
    }

    /// Determines if the active container is the root container
    pub fn active_is_root(&self) -> bool {
        if let Some(active_ix) = self.active_container {
            self.tree.is_root_container(active_ix)
        } else {
            false
        }
    }

    /// Make a new output container with the given WlcOutput.
    ///
    /// A new workspace is automatically added to the output, to ensure
    /// consistency with the tree. By default, it sets this new workspace to
    /// be workspace "1". This will later change to be the first available
    /// workspace if using i3-style workspaces.
    pub fn add_output(&mut self, output: WlcOutput) {
        trace!("Adding new output with {:?}", output);
        let root_index = self.tree.root_ix();
        let output_ix = self.tree.add_child(root_index,
                                          Container::new_output(output));
        self.active_container = Some(self.init_workspace("1".to_string(), output_ix));
        self.validate();
    }

    /// Add a new view container with the given WlcView to the active container
    pub fn add_view(&mut self, view: WlcView) {
        if let Some(mut active_ix) = self.active_container {
            let parent_ix = self.tree.parent_of(active_ix)
                .expect("Active container had no parent");
            // Get the previous position before correcting the container
            let prev_pos = *self.tree.get_edge_weight_between(parent_ix, active_ix)
                .expect("Could not get edge weight between active and active parent")
                + 1;
            if self.tree[active_ix].get_type() == ContainerType::View {
                active_ix = self.tree.parent_of(active_ix)
                    .expect("View had no parent");
            }
            let view_ix = self.tree.add_child(active_ix,
                                              Container::new_view(view));
            self.tree.set_child_pos(view_ix, prev_pos);
            self.active_container = Some(view_ix);
        }
        self.validate();
    }

    //// Remove a view container from the tree
    pub fn remove_view(&mut self, view: &WlcView) -> Result<(), String> {
        if let Some(view_ix) = self.tree.descendant_with_handle(self.tree.root_ix(), view) {
            self.remove_view_or_container(view_ix);
            self.validate();
            Ok(())
        } else {
            self.validate();
            Err(format!("Could not find descendant with handle {:#?} to remove", view))
        }
    }

    /// Removes the current active container
    pub fn remove_active(&mut self) {
        if let Some(active_ix) = self.active_container {
            self.remove_view_or_container(active_ix);
        }
    }

    /// Special code to handle removing a View or Container.
    /// We have to ensure that we aren't invalidating the active container
    /// when we remove a view or container.
    fn remove_view_or_container(&mut self, node_ix: NodeIndex) {
        // Only the root container has a non-container parent, and we can't remove that
        if let Some(mut parent_ix) = self.tree.ancestor_of_type(node_ix,
                                                                    ContainerType::Container) {
            // If it'll move, fix that before that happens
            if self.tree.is_last_ix(parent_ix) {
                parent_ix = node_ix;
            }
            // If the active container is *not* being removed,
            // we must ensure that it won't be invalidated by the move
            // (i.e: if it is the last index)
            if self.active_container.map(|c| c != node_ix).unwrap_or(false) {
                if self.tree.is_last_ix(self.active_container.unwrap()) {
                    self.active_container = Some(node_ix);
                }
            }
            let container = self.tree.remove(node_ix)
                .expect("Could not remove container");
            match container {
                Container::View { ref handle, .. } => {
                    handle.close();
                },
                Container::Container { .. } => {},
                _ => unreachable!()
            };
            self.focus_on_next_container(parent_ix);
            // Remove parent container if it is a non-root container and has no other children
            match self.tree[parent_ix].get_type() {
                ContainerType::Container => {
                    if self.tree.can_remove_empty_parent(parent_ix) {
                        self.remove_view_or_container(parent_ix);
                    }
                }
                _ => {},
            }
            trace!("Removed container {:?}, index {:?}", container, node_ix);
        }
        self.validate();
    }

    /// Gets a workspace by name or creates it
    fn get_or_make_workspace(&mut self, name: &str) -> NodeIndex {
        let active_index = self.active_ix_of(ContainerType::Output).expect("get_or_make_wksp: Couldn't get output");
        let workspace_ix = self.tree.workspace_ix_by_name(name).unwrap_or_else(|| {
            let root_ix = self.init_workspace(name.to_string(), active_index);
            self.tree.parent_of(root_ix)
                .expect("Workspace was not properly initialized with a root container")
        });
        self.validate();
        workspace_ix
    }

    /// Initializes a workspace and gets the index of the root container
    fn init_workspace(&mut self, name: String, output_ix: NodeIndex)
                      -> NodeIndex {
        let size = self.tree.get(output_ix)
            .expect("init_workspace: invalid output").get_geometry()
            .expect("init_workspace: no geometry for output").size;
        let worksp = Container::new_workspace(name.to_string(), size.clone());

        trace!("Adding workspace {:?}", worksp);
        let worksp_ix = self.tree.add_child(output_ix, worksp);
        let geometry = Geometry {
            size: size, origin: Point { x: 0, y: 0 }
        };
        let container_ix = self.tree.add_child(worksp_ix,
                                               Container::new_container(geometry));
        self.validate();
        container_ix
    }

    /// Adds the container with the node index as a child.
    /// The node at the node index is removed and
    /// made a child of the new container node.
    ///
    /// The new container has the same edge weight as the child that is passed in.
    fn add_container(&mut self, container: Container, child_ix: NodeIndex) {
        let parent_ix = self.tree.parent_of(child_ix)
            .expect("Node had no parent");
        let old_weight = *self.tree.get_edge_weight_between(parent_ix, child_ix)
            .expect("parent and children were not connected");
        let new_container_ix = self.tree.add_child(parent_ix, container);
        self.tree.move_node(child_ix, new_container_ix);
        self.tree.set_child_pos(new_container_ix, old_weight);
        self.active_container = Some(new_container_ix);
        self.validate();
    }

    /// Remove a container from the tree.
    /// The active container is preserved after this operation,
    /// if it was moved then it's new index will be reflected in the structure
    ///
    /// Note that because this causes N indices to be changed (where N is the
    /// number of descendants of the container), any node indices should be
    /// considered invalid after this operation (except for the active_container)
    fn remove_container(&mut self, container_ix: NodeIndex) {
        let mut children = self.tree.all_descendants_of(&container_ix);
        // add current container to the list as well
        children.push(container_ix);
        for node_ix in children {
            trace!("Removing index {:?}: {:?}", node_ix, self.tree[node_ix]);
            match self.tree[node_ix] {
                Container::View { .. } | Container::Container { .. } => {
                    self.remove_view_or_container(node_ix);
                },
                _ => { self.tree.remove(node_ix); },
            }
        }
        self.validate();
    }

    /// Focus on the container relative to the active container.
    ///
    /// If Horizontal, left and right will move within siblings.
    /// If Vertical, up and down will move within siblings.
    /// Other wise, it moves to the next sibling of the parent container.
    ///
    /// If the edge of the children is hit, it does not wrap around,
    /// but moves between ancestor siblings.
    pub fn move_focus(&mut self, direction: Direction) {
        if let Some(prev_active_ix) = self.active_container {
            self.active_container = Some(self.move_focus_recurse(prev_active_ix, direction)
                .unwrap_or(prev_active_ix));
            match self.tree[self.active_container.unwrap()] {
                Container::View { ref handle, .. } => handle.focus(),
                _ => warn!("move_focus returned a non-view, cannot focus")
            }
        } else {
            warn!("Cannot move active focus when not there is no active container");
        }
        self.validate();
    }
    fn move_focus_recurse(&mut self, node_ix: NodeIndex, direction: Direction) -> Result<NodeIndex, ()> {
        match self.tree[node_ix].get_type() {
            ContainerType::View | ContainerType::Container => { /* continue */ },
            _ => return Err(())
        }
        let parent_ix = self.tree.parent_of(node_ix)
            .expect("Active ix had no parent");
        match self.tree[parent_ix] {
            Container::Container { layout, .. } => {
                match (layout, direction) {
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
                                        // Get the first view we can find in the container
                                        let first_view = self.tree.descendant_of_type(new_active_ix, ContainerType::View)
                                            .expect("Could not find view in ancestor sibling container");
                                        trace!("Moving to different view {:?} in container {:?}",
                                               self.tree[first_view], self.tree[new_active_ix]);
                                        return Ok(first_view);
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
                return Err(());
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
    fn focus_on_next_container(&mut self, mut parent_ix: NodeIndex) {
        while self.tree.node_type(parent_ix)
            .expect("Node not part of the tree") != ContainerType::Workspace {
            if let Some(view_ix) = self.tree.descendant_of_type_right(parent_ix,
                                                           ContainerType::View) {
                match self.tree[view_ix]
                                    .get_handle().expect("view had no handle") {
                    Handle::View(view) => view.focus(),
                    _ => panic!("View had an output handle")
                }
                trace!("Active container set to view at {:?}", view_ix);
                self.active_container = Some(view_ix);
                return;
            }
            parent_ix = self.tree.ancestor_of_type(parent_ix,
                                                   ContainerType::Container)
                .unwrap_or_else(|| {
                    self.tree.ancestor_of_type(parent_ix, ContainerType::Workspace)
                        .expect("Container was not part of a workspace")
                });
        }
        // If this is reached, parent is workspace
        let container_ix = self.tree.children_of(parent_ix)[0];
        let root_c_children = self.tree.children_of(container_ix);
        if root_c_children.len() > 0 {
            let new_active_ix = self.tree.descendant_of_type(root_c_children[0],
                                                             ContainerType::View)
                .unwrap_or(root_c_children[0]);
            self.active_container = Some(new_active_ix);
            match self.tree[new_active_ix] {
                Container::View { ref handle, .. } => handle.focus(),
                _ => {}
            };
            return;
        }
        trace!("Active container set to container {:?}", container_ix);
        self.active_container = Some(container_ix);

        // Update focus to new container
        self.get_active_container().map(|con| match *con {
            Container::View { ref handle, .. } => handle.focus(),
            Container::Container { .. } => WlcView::root().focus(),
            _ => panic!("Active container not view or container!")
        });
    }

    /// Changes the layout of the active container to the given layout.
    /// If the active container is a view, a new container is added with the given
    /// layout type.
    pub fn toggle_active_layout(&mut self, new_layout: Layout) {
        if let Some(active_ix) = self.active_container {
            let parent_ix = self.tree.parent_of(active_ix)
                .expect("Active container had no parent");
            if self.tree.is_root_container(active_ix) {
                self.set_layout(active_ix, new_layout);
                return
            }
            if self.tree.children_of(parent_ix).len() == 1 {
                self.set_layout(parent_ix, new_layout);
                return
            }

            let active_geometry = self.get_active_container()
                .expect("Could not get the active container")
                .get_geometry().expect("Active container had no geometry");

            let mut new_container = Container::new_container(active_geometry);
            new_container.set_layout(new_layout).ok();
            self.add_container(new_container, active_ix);
            // add_container sets the active container to be the new container
            self.active_container = Some(active_ix);
        }
        self.validate();
    }

    /// Normalizes the geometry of a view to be the same size as it's siblings,
    /// based on the parent container's layout, at the 0 point of the parent container.
    /// Note this does not auto-tile, only modifies this one view.
    ///
    /// Useful if a container's children want to be evenly distributed, or a new view
    /// is being added.
    pub fn normalize_view(&mut self, view: WlcView) {
        if let Some(view_ix) = self.tree.descendant_with_handle(self.tree.root_ix(), &view) {
            self.normalize_container(view_ix);
        }
        self.validate();
    }

    /// Gets the active container and toggles it based on the following rules:
    /// * If horizontal, make it vertical
    /// * else, make it horizontal
    /// This method does *NOT* update the actual views geometry, that needs to be
    /// done separately by the caller
    pub fn toggle_active_horizontal(&mut self) {
        if let Some(active_ix) = self.active_ix_of(ContainerType::Container) {
            trace!("Toggling {:?} to be horizontal or vertical...", self.tree[active_ix]);
            match self.tree[active_ix] {
                Container::Container { ref mut layout, .. } => {
                    match *layout {
                        Layout::Horizontal => {
                            trace!("Toggling to be vertical");
                            *layout = Layout::Vertical
                        }
                        _ => {
                            trace!("Toggling to be horizontal");
                            *layout = Layout::Horizontal
                        }
                    }
                },
                _ => unreachable!()
            }
        } else {
            error!("No active container")
        }
        self.validate();
    }

    /// Switch to the specified workspace
    pub fn switch_to_workspace(&mut self, name: &str) {
        if self.active_container.is_none() {
            warn!("No active container, cannot switch");
            return;
        }
        // Set old workspace to be invisible
        let old_worksp_ix: NodeIndex;
        if let Some(index) = self.active_ix_of(ContainerType::Workspace) {
            old_worksp_ix = index;
            trace!("Switching to workspace {}", name);
            self.tree.set_family_visible(old_worksp_ix, false);
        } else {
            warn!("Could not find old workspace, could not set invisible");
            return;
        }
        // Get the new workspace, or create one if it doesn't work
        let mut workspace_ix = self.get_or_make_workspace(name);
        if old_worksp_ix == workspace_ix {
            return;
        }
        // Set the new one to visible
        self.tree.set_family_visible(workspace_ix, true);
        // Delete the old workspace if it has no views on it
        self.active_container = None;
        if self.tree.descendant_of_type(old_worksp_ix, ContainerType::View).is_none() {
            trace!("Removing workspace: {:?}", self.tree[old_worksp_ix].get_name()
                   .expect("Workspace had no name"));
            self.remove_container(old_worksp_ix);
        }
        workspace_ix = self.tree.workspace_ix_by_name(name)
            .expect("Workspace we just made was deleted!");
        self.focus_on_next_container(workspace_ix);
        self.validate();
    }

    /// Moves the current active container to a new workspace
    pub fn send_active_to_workspace(&mut self, name: &str) {
        // Ensure focus
        if let Some(active_ix) = self.active_container {
            let curr_work_ix = self.active_ix_of(ContainerType::Workspace)
                .expect("send_active: Not currently in a workspace!");
            if active_ix == self.tree.children_of(curr_work_ix)[0] {
                warn!("Tried to move the root container of a workspace, aborting move");
                return;
            }
            let next_work_ix = self.get_or_make_workspace(name);

            // Check if the workspaces are the same
            if next_work_ix == curr_work_ix {
                trace!("Attempted to move a view to the same workspace {}!", name);
                return;
            }
            self.tree.set_family_visible(curr_work_ix, false);

            // Save the parent of this view for focusing
            let maybe_active_parent = self.tree.parent_of(active_ix);

            // Get the root container of the next workspace
            let next_work_children = self.tree.children_of(next_work_ix);
            if cfg!(debug_assertions) {
                assert!(next_work_children.len() == 1,
                        "Next workspace has multiple roots!");
            }
            let next_work_root_ix = next_work_children[0];

            // Move the container
            info!("Moving container {:?} to workspace {}",
                self.get_active_container(), name);
            self.tree.move_node(active_ix, next_work_root_ix);

            // Update the active container
            if let Some(parent_ix) = maybe_active_parent {
                let ctype = self.tree.node_type(parent_ix).unwrap_or(ContainerType::Root);
                if ctype == ContainerType::Container {
                    self.focus_on_next_container(parent_ix);
                } else {
                    trace!("Send to container invalidated a NodeIndex: {:?} to {:?}",
                    parent_ix, ctype);
                }
                if self.tree.can_remove_empty_parent(parent_ix) {
                    self.remove_view_or_container(parent_ix);
                }
            }
            else {
                self.focus_on_next_container(curr_work_ix);
            }

            self.tree.set_family_visible(curr_work_ix, true);

            self.normalize_container(active_ix);
        }
        let root_ix = self.tree.root_ix();
        self.layout(root_ix);
        self.validate();
    }

    /// Validates the tree
    #[cfg(debug_assertions)]
    pub fn validate(&self) {
        // Recursive method to ensure child/parent nodes are connected
        fn validate_node_connections(this: &LayoutTree, parent_ix: NodeIndex) {
            for child_ix in this.tree.children_of(parent_ix) {
                let child_parent = this.tree.parent_of(child_ix)
                    .expect("connections: Child did not point to parent!");
                if child_parent != parent_ix {
                    error!("Child at {:?} has parent {:?}, expected {:?}",
                           child_ix, child_parent, parent_ix);
                    trace!("The tree: {:#?}", this);
                    panic!()
                }
                validate_node_connections(this, child_ix);
            }
        }

        validate_node_connections(self, self.tree.root_ix());

        // Ensure active container is in tree and of right type
        if let Some(active_ix) = self.active_container {
            if self.active_container.is_some() {
                let active = self.get_active_container()
                    .expect("active_container points to invalid node");
                match active.get_type() {
                    ContainerType::View | ContainerType::Container => {},
                    _ => panic!("Active container was not view or container")
                }
                // Check active container in tree
                if self.tree.ancestor_of_type(active_ix, ContainerType::Root).is_none() {
                    error!("Active container @ {:?} is not part of tree!", active_ix);
                    error!("Active container is {:?}", active);
                    trace!("The tree: {:#?}", self);
                    panic!()
                }
            }
        }

        // Ensure workspace have at least one child
        for output_ix in self.tree.children_of(self.tree.root_ix()) {
            for workspace_ix in self.tree.children_of(output_ix) {
                if self.tree.children_of(workspace_ix).len() == 0 {
                    error!("Workspace {:#?} has no children",
                           self.tree[workspace_ix]);
                    trace!("The tree: {:#?}", self);
                    panic!()
                }
                for container_ix in self.tree.all_descendants_of(&workspace_ix) {
                    match self.tree[container_ix] {
                        Container::Container { .. } => {
                            assert!(! self.tree.can_remove_empty_parent(container_ix));
                        },
                        Container::View { .. } => {
                        }
                        _ => panic!("Non-view/container as descendant of a workspace!")
                    }
                }
            }
        }
    }

    #[cfg(not(debug_assertions))]
    pub fn validate(&self) {}

}


#[cfg(test)]
mod tests {
    use super::super::super::LayoutTree;
    use super::super::super::container::*;
    use super::super::super::graph_tree::InnerTree;
    use super::*;
    use rustwlc::*;

    /// Makes a very basic tree.
    /// There is only one output,
    /// Two workspaces,
    /// First workspace has a single view in the root container,
    /// second workspace has a container with two views in it
    /// (the container is a child of the root container).
    ///
    /// The active container is the only view in the first workspace
    #[allow(unused_variables)]
    fn basic_tree() -> LayoutTree {
        let mut tree = InnerTree::new();
        let fake_view_1 = WlcView::root();
        let fake_output = fake_view_1.clone().as_output();
        let root_ix = tree.root_ix();
        let fake_size = Size { h: 800, w: 600 };
        let fake_geometry = Geometry {
            size: fake_size.clone(),
            origin: Point { x: 0, y: 0 }
        };

        let output_ix = tree.add_child(root_ix, Container::new_output(fake_output));
        let workspace_1_ix = tree.add_child(output_ix,
                                                Container::new_workspace("1".to_string(),
                                                                   fake_size.clone()));
        let root_container_1_ix = tree.add_child(workspace_1_ix,
                                                Container::new_container(fake_geometry.clone()));
        let workspace_2_ix = tree.add_child(output_ix,
                                                Container::new_workspace("2".to_string(),
                                                                     fake_size.clone()));
        let root_container_2_ix = tree.add_child(workspace_2_ix,
                                                Container::new_container(fake_geometry.clone()));
        /* Workspace 1 containers */
        let wkspc_1_view = tree.add_child(root_container_1_ix,
                                                Container::new_view(fake_view_1.clone()));
        /* Workspace 2 containers */
        let wkspc_2_container = tree.add_child(root_container_2_ix,
                                                Container::new_container(fake_geometry.clone()));
        let wkspc_2_sub_view_1 = tree.add_child(wkspc_2_container,
                                                Container::new_view(fake_view_1.clone()));
        let wkspc_2_sub_view_2 = tree.add_child(wkspc_2_container,
                                                Container::new_view(fake_view_1.clone()));

        let layout_tree = LayoutTree {
            tree: tree,
            active_container: Some(wkspc_1_view)
        };
        layout_tree
    }

    #[test]
    /// Ensures that getting the active container always returns either
    /// a view, a container, or nothing.
    fn active_container_tests() {
        let mut simple_tree = basic_tree();
        /* Standard active_container getters */
        {
            let active_container = simple_tree.get_active_container().unwrap();
            let view_ancestor_ix = simple_tree.active_ix_of(ContainerType::View).unwrap();
            assert_eq!(*active_container, simple_tree.tree[view_ancestor_ix]);
            match *active_container {
                Container::View { .. }| Container::Container { .. }=> {},
                _ => panic!("Active container was not a view or container!")
            }
        }
        {
            let active_container_mut = simple_tree.get_active_container_mut().unwrap();
            match *active_container_mut {
                Container::View { .. }| Container::Container { .. }=> {},
            _ => panic!("Active container was not a view or container!")
            }
        }
        /* Active workspace getters */
        {
            let active_workspace = simple_tree.get_active_workspace().unwrap();
            let workspace_ancestor_ix = simple_tree.active_ix_of(ContainerType::Workspace).unwrap();
            assert_eq!(*active_workspace, simple_tree.tree[workspace_ancestor_ix]);
            match *active_workspace {
                Container::Workspace { ref name, .. } => {
                    assert_eq!(name.as_str(), "1")
                },
                _ => panic!("get_active_workspace did not return a workspace")
            }
        }
        {
            let active_workspace_mut = simple_tree.get_active_workspace_mut().unwrap();
            match *active_workspace_mut {
                Container::Workspace { ref name, .. } => {
                assert_eq!(name.as_str(), "1")
            },
            _ => panic!("get_active_workspace did not return a workspace")
            }
        }
        /* Active output getters */
        {
            let active_output = simple_tree.get_active_output().unwrap();
            let output_ancestor_ix = simple_tree.active_ix_of(ContainerType::Output).unwrap();
            assert_eq!(*active_output, simple_tree.tree[output_ancestor_ix]);
            match *active_output {
                Container::Output { ref handle, .. } => {
                    assert_eq!(*handle, WlcView::root().as_output());
                }
                _ => panic!("get_active_output did not return an output")
            }
        }
        {
            let active_output_mut = simple_tree.get_active_output_mut().unwrap();
            match *active_output_mut {
                Container::Output { ref handle, .. } => {
                    assert_eq!(*handle, WlcView::root().as_output());
                }
                _ => panic!("get_active_output did not return an output")
            }
        }
    }

    #[test]
    fn active_container_test() {
        let mut tree = basic_tree();
        tree.active_container = None;
        assert_eq!(tree.get_active_container(), None);
        assert_eq!(tree.get_active_parent(), None);
        assert_eq!(tree.active_ix_of(ContainerType::View), None);
        assert_eq!(tree.active_ix_of(ContainerType::Container), None);
        assert_eq!(tree.active_ix_of(ContainerType::Workspace), None);
        assert_eq!(tree.active_ix_of(ContainerType::Output), None);
        assert_eq!(tree.active_ix_of(ContainerType::Root), None);
        tree.set_active_container(WlcView::root());
        let view_ix = tree.tree.descendant_with_handle(tree.tree.root_ix(), &WlcView::root()).unwrap();
        assert_eq!(tree.active_container, Some(view_ix));
        tree.unset_active_container();
        assert_eq!(tree.get_active_container(), None);
        assert_eq!(tree.active_container, None);
    }

    #[test]
    /// Tests workspace functions, ensuring we can get workspaces and new
    /// ones are properly generated with a root container when we request one
    /// that doesn't yet exist
    fn workspace_tests() {
        let mut tree = basic_tree();
        /* Simple workspace access tests */
        let workspace_1_ix = tree.tree.workspace_ix_by_name("1")
            .expect("Workspace 1 did not exist");
        assert_eq!(tree.tree[workspace_1_ix].get_type(), ContainerType::Workspace);
        assert_eq!(tree.tree[workspace_1_ix].get_name().unwrap(), "1");
        let workspace_2_ix = tree.tree.workspace_ix_by_name("2")
            .expect("Workspace 2 did not exist");
        assert_eq!(tree.tree[workspace_2_ix].get_type(), ContainerType::Workspace);
        assert_eq!(tree.tree[workspace_2_ix].get_name().unwrap(), "2");
        assert!(tree.tree.workspace_ix_by_name("3").is_none(),
                "Workspace three existed, expected it not to");
        /* init workspace tests */
        let output_ix = tree.active_ix_of(ContainerType::Output)
            .expect("No active output");
        let active_3_ix = tree.init_workspace("3".to_string(), output_ix);
        let workspace_3_ix = tree.tree.parent_of(active_3_ix).unwrap();
        assert!(tree.tree.workspace_ix_by_name("3").is_some(),
                "Workspace three still does not exist, even though we just initialized it");
        assert_eq!(tree.tree[workspace_3_ix].get_type(), ContainerType::Workspace);
        assert_eq!(tree.tree[workspace_3_ix].get_name().unwrap(), "3");
    }

    #[test]
    /// Tests the view functions
    fn view_tests() {
        let mut tree = basic_tree();
        let active_container = tree.active_container.expect("No active container");
        let parent_container = tree.tree.parent_of(active_container).unwrap();
        // When the active container is a view, add it as a sibling
        assert_eq!(tree.tree.children_of(parent_container).len(), 1);
        let old_active_view = tree.active_ix_of(ContainerType::View)
            .expect("Active container was not a view");
        tree.add_view(WlcView::root());
        assert_eq!(tree.tree.children_of(parent_container).len(), 2);
        assert!(! (old_active_view == tree.active_ix_of(ContainerType::View).unwrap()));
        tree.remove_view(&WlcView::root());
        assert_eq!(tree.active_ix_of(ContainerType::View).unwrap(), old_active_view);
        assert_eq!(tree.tree.children_of(parent_container).len(), 1);
        for _ in 1..2 {
            tree.remove_view(&WlcView::root());
        }
    }

    #[test]
    fn remove_active_test() {
        let mut tree = basic_tree();
        let root_container = tree.tree[tree.tree.parent_of(tree.active_container.unwrap()).unwrap()].clone();
        tree.remove_active();
        assert_eq!(tree.tree[tree.active_container.unwrap()], root_container);
    }

    #[test]
    fn add_output_test() {
        let mut tree = basic_tree();
        let new_output = WlcView::root().as_output();
        tree.add_output(new_output);
        let output_ix = tree.active_ix_of(ContainerType::Output).unwrap();
        let handle = match tree.tree[output_ix].get_handle().unwrap() {
            Handle::Output(output) => output,
            _ => panic!()
        };
        assert_eq!(handle, new_output);
        let workspace_ix = tree.tree.descendant_of_type(output_ix, ContainerType::Workspace).unwrap();
        assert_eq!(tree.tree[workspace_ix].get_name().unwrap(), "1");
        let active_ix = tree.active_container.unwrap();
        assert_eq!(tree.tree.parent_of(active_ix).unwrap(), workspace_ix);
        assert_eq!(tree.tree.children_of(active_ix).len(), 0);
    }

    #[test]
    /// Tests that we can remove the active container and have it properly reset
    fn basic_removal() {
        let mut tree = basic_tree();
        let active_view_ix = tree.active_container
            .expect("No active container on basic tree");
        assert!(tree.tree[active_view_ix].get_type() == ContainerType::View,
                "Active container was not a view");
        let workspace_of_active = tree.tree.ancestor_of_type(active_view_ix,
                                                             ContainerType::Workspace)
            .expect("View not part of workspace");
        // The next active container should be the root container of this workspace
        let new_active_container_ix = &tree.tree.children_of(workspace_of_active)[0];

        tree.remove_view_or_container(active_view_ix);
        let new_active_container = tree.active_container
            .expect("Remove view invalidated the active container");
        assert_eq!(new_active_container, *new_active_container_ix);

    }

    #[test]
    fn toggle_layout_test() {
        {
            let mut tree = basic_tree();
            let root_container = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
            tree.active_container = Some(root_container);
            assert!(tree.tree.is_root_container(root_container));
            let layout = match tree.tree[root_container] {
                Container::Container { ref layout, .. } => layout.clone(),
                _ => panic!()
            };
            // default layout
            assert_eq!(layout, Layout::Horizontal);
            for new_layout in &[Layout::Vertical, Layout::Horizontal,
                            Layout::Tabbed, Layout::Stacked] {
                tree.toggle_active_layout(*new_layout);
                let layout = match tree.tree[root_container] {
                    Container::Container { ref layout, .. } => layout.clone(),
                    _ => panic!()
                };
                assert_eq!(layout, *new_layout);
            }
        }
        /* Now test wrapping the active container in a new container */
        {
            let mut tree = basic_tree();
            let active_ix = tree.active_container.unwrap();
            let active_container = tree.tree[active_ix].clone();
            let old_parent = tree.tree[tree.tree.parent_of(active_ix).unwrap()]
                .clone();
            let old_layout = match old_parent {
                Container::Container { ref layout, ..} => layout.clone(),
                _ => panic!()
            };
            assert_eq!(old_layout, Layout::Horizontal);
            tree.toggle_active_layout(Layout::Vertical);
            // should still be focused on the previous container.
            // though the active index might be different
            let active_ix = tree.active_container.unwrap();
            assert_eq!(active_container, tree.tree[active_ix]);
            let new_parent = tree.tree[tree.tree.parent_of(active_ix).unwrap()]
                .clone();
            let new_layout = match new_parent {
                Container::Container { ref layout, ..} => layout.clone(),
                _ => panic!()
            };
            assert!(old_parent != new_parent);
            assert_eq!(new_layout, Layout::Vertical);
        }
    }

    #[test]
    fn add_container_test() {
        let mut tree = basic_tree();
        let active_ix = tree.active_container.unwrap();
        let parent_ix = tree.tree.parent_of(active_ix).unwrap();
        let old_edge_weight = *tree.tree.get_edge_weight_between(parent_ix, active_ix)
            .unwrap();
        // First and only child, so the edge weight is 1
        assert_eq!(old_edge_weight, 1);
        let geometry = Geometry {
            origin: Point { x: 0, y: 0},
            size: Size { w: 0, h: 0}
        };
        let new_container = Container::new_container(geometry);
        tree.add_container(new_container, active_ix);
        let new_active_ix = tree.active_container.unwrap();
        // The view moved, since it was placed in the new container
        assert!(active_ix != new_active_ix);
        let new_container_ix = tree.tree.parent_of(new_active_ix).unwrap();
        let parent_ix = tree.tree.parent_of(new_container_ix).unwrap();
        let new_edge_weight = *tree.tree.get_edge_weight_between(parent_ix, new_container_ix)
            .unwrap();
        assert_eq!(new_edge_weight, old_edge_weight);

    }

    #[test]
    fn non_root_container_auto_removal_test() {
        let mut tree = basic_tree();
        tree.switch_to_workspace("2");
        /* Remove first View */
        let root_container = tree.tree.children_of(tree.active_ix_of(ContainerType::Workspace)
                                                   .expect("No active workspace"))[0];
        let num_children = tree.tree.children_of(root_container).len();
        assert_eq!(num_children, 1);
        let active_view_ix = tree.active_container.unwrap();
        assert_eq!(tree.tree[active_view_ix].get_type(), ContainerType::View);
        tree.remove_view_or_container(active_view_ix);
        /* Remove the other view*/
        let active_view_ix = tree.active_container.unwrap();
        assert_eq!(tree.tree[active_view_ix].get_type(), ContainerType::View);
        tree.remove_view_or_container(active_view_ix);
        /* This should remove the other container,
        the count of the root container should be 0 */
        let active_ix = tree.active_container.unwrap();
        assert!(tree.tree.is_root_container(active_ix));
        let root_container = tree.tree.children_of(tree.active_ix_of(ContainerType::Workspace)
                                                   .expect("No active workspace"))[0];
        let num_children = tree.tree.children_of(root_container).len();
        assert_eq!(num_children, 0);
    }

    #[test]
    fn move_to_workspace_test() {
        // NOTE Need to test adding to workspace with stuff already in that workspace
        let mut tree = basic_tree();
        /* Make sure sending to the current workspace does nothing */
        let old_view = tree.tree[tree.active_container.unwrap()].clone();
        tree.send_active_to_workspace("1");
        assert_eq!(old_view, tree.tree[tree.active_container.unwrap()]);
        //let old_view = tree.tree[tree.active_container.unwrap()].clone();
        tree.send_active_to_workspace("3");
        // Trying to send the root container does nothing
        tree.send_active_to_workspace("3");
        let active_ix = tree.active_container.unwrap();
        assert!(tree.tree.is_root_container(active_ix));
        tree.switch_to_workspace("3");
        let active_ix = tree.active_container.unwrap();
        // Switch to new workspace, should be focused on the old view
        assert_eq!(old_view, tree.tree[active_ix]);
    }

    #[test]
    fn auto_workspace_adding() {
        let mut tree = basic_tree();
        let output = tree.active_ix_of(ContainerType::Output).unwrap();
        // there are two workspaces at the beginning, 1 and 2
        assert_eq!(tree.tree.children_of(output).len(), 2);
        tree.switch_to_workspace("1");
        // Switching to current doesn't change that
        assert_eq!(tree.tree.children_of(output).len(), 2);
        // Switching to other doesn't either
        tree.switch_to_workspace("2");
        assert_eq!(tree.tree.children_of(output).len(), 2);
        // This does add the new one
        tree.switch_to_workspace("3");
        assert_eq!(tree.tree.children_of(output).len(), 3);
    }


    #[test]
    /// Ensures that toggle horizontal key (<Leader> + e) does the same thing as it does in i3.
    /// To reiterate: it should always make the active view's parent container( or the container
    /// itself if the active container is a container, not a view) have the horizontal layout
    /// _unless_ it's already horizontal, in which case the layout should be vertical
    fn tiling_toggle_key() {
        let mut tree = basic_tree();
        // active container is the first view, so it should just change it's root.
        match *tree.get_active_parent().unwrap() {
            Container::Container { ref layout, .. } => {
                // default is horizontal
                assert_eq!(*layout, Layout::Horizontal)
            },
            _ => unreachable!()
        }
        tree.toggle_active_horizontal();
        match *tree.get_active_parent().unwrap() {
            Container::Container { ref layout, .. } => {
                // default is horizontal
                assert_eq!(*layout, Layout::Vertical)
            },
            _ => unreachable!()
        }
        // and back again
        tree.toggle_active_horizontal();
        match *tree.get_active_parent().unwrap() {
            Container::Container { ref layout, .. } => {
                // default is horizontal
                assert_eq!(*layout, Layout::Horizontal)
            },
            _ => unreachable!()
        }
    }

    #[test]
    fn move_focus_simple_test() {
        let mut tree = basic_tree();
        let directions = [Direction::Up, Direction::Right,
                          Direction::Down, Direction::Left];
        let old_active_ix = tree.active_container.clone();
        tree.active_container = None;
        for direction in &directions {
            // should do nothing when there is no active container
            tree.move_focus(*direction);
            assert_eq!(tree.active_container, None);
        }
        tree.active_container = old_active_ix;
        for direction in &directions {
            // should do nothing when there are no other views to move to
            tree.move_focus(*direction);
            assert_eq!(tree.active_container, old_active_ix);
        }
        // set to root container
        let root_container_ix = tree.tree.parent_of(old_active_ix.unwrap()).unwrap();
        assert!(tree.tree.is_root_container(root_container_ix));
        tree.active_container = Some(root_container_ix);
        for direction in &directions {
            tree.move_focus(*direction);
            assert_eq!(tree.active_container, Some(root_container_ix));
        }
    }

    #[test]
    fn move_focus_complex_test() {
        let mut tree = basic_tree();
        tree.switch_to_workspace("2");
        // We are focused on the far left container
        let left_ix = tree.active_container.unwrap();
        // Get the next one
        tree.move_focus(Direction::Right);
        // Make sure we moved
        let right_ix = tree.active_container.unwrap();
        assert!(left_ix != right_ix);
        // make a vertical container here, try to move back to the original
        let new_container = tree.tree[right_ix].clone();
        tree.toggle_active_layout(Layout::Vertical);
        assert_eq!(new_container, tree.tree[tree.active_container.unwrap()]);
        // Add a new view, it'll be below us
        tree.add_view(WlcView::root());
        // Move up, should be on the original view still
        tree.move_focus(Direction::Up);
        assert_eq!(new_container, tree.tree[tree.active_container.unwrap()]);
        // Move Right, should be right where we are, nothing to the right
        tree.move_focus(Direction::Right);
        assert_eq!(new_container, tree.tree[tree.active_container.unwrap()]);
        // Move left, be back on the very first one
        tree.move_focus(Direction::Left);
        assert_eq!(left_ix, tree.active_container.unwrap());
        // Move left, be back on the very first one
    }

    #[test]
    fn switch_to_workspace_test() {
        let mut tree = basic_tree();
        let old_active = tree.active_container.clone();
        let current_workspace_ix = tree.active_ix_of(ContainerType::Workspace).unwrap();
        tree.active_container = None;
        tree.switch_to_workspace("3");
        // didn't move, because we have no active container
        tree.active_container = old_active;
        assert_eq!(tree.active_ix_of(ContainerType::Workspace).unwrap(), current_workspace_ix);
        tree.switch_to_workspace("1");
        // didn't move, because we aren't going anywhere different (same workspace)
        assert_eq!(tree.active_ix_of(ContainerType::Workspace).unwrap(), current_workspace_ix);
        tree.active_container = tree.active_ix_of(ContainerType::Output);
        tree.switch_to_workspace("3");
        // didn't move, because we aren't focused on something with a workspace
        tree.active_container = old_active;
        assert_eq!(tree.active_ix_of(ContainerType::Workspace).unwrap(), current_workspace_ix);
    }

    #[test]
    fn active_is_root_test() {
        let mut tree = basic_tree();
        assert_eq!(tree.active_is_root(), false);
        tree.remove_active();
        assert_eq!(tree.active_is_root(), true);
    }
}
