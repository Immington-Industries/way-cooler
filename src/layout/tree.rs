//! Main module to handle the layout.
//! This is where the i3-specific code is.

use std::sync::{Mutex, MutexGuard, TryLockError};
use std::ptr;

use petgraph::graph::NodeIndex;

use layout::container::{Container, Handle, ContainerType};
use rustwlc::{WlcView, WlcOutput, Geometry, Point};

use layout::graph_tree::Tree;

/// Error for trying to lock the tree
pub type TreeErr = TryLockError<MutexGuard<'static, LayoutTree>>;
/// Result for locking the tree
pub type TreeResult = Result<MutexGuard<'static, LayoutTree>, TreeErr>;

const ERR_BAD_TREE: &'static str = "Layout tree was in an invalid configuration";

/* An example Tree:

      Root
        |
    ____|____
   /         \
   |         |
 Output    Output
   |         |
 Workspace   |
   |        / \
   |       /   \
   | Workspace Workspace
   |     |         |
   |  Container Container
 Container
    |
   / \
  /   \
  |    \
  |     \
  |      \
  |       \
 Container \
     |      \
   View    View
 */

/// The layout tree builds on top of the graph_tree.
///
/// There are various invariants that the tree upholds:
///
///   + Root
///       - There is only one Root
///       - The root can only have Outputs (monitors) as children
///   + Output
///       - An Output must have at least one Workspace associated with it
///       - An Output must be associated with a WlcOutput (real monitor)
///       - An Output can only have Workspaces as children
///   + Workspace
///       - A Workspace must have at least one Container, even if it doesn't
///         contain any views
///       - A Workspace can only have Containers as children
///   + Container
///       - A Container can only have other Containers or Views as children
///   + View
///       - A View must be associated with a WlcView
///       - A View cannot have any children
#[derive(Debug)]
pub struct LayoutTree {
    tree: Tree,
    active_container: Option<NodeIndex>
}

lazy_static! {
    static ref TREE: Mutex<LayoutTree> = {
        Mutex::new(LayoutTree {
            tree: Tree::new(),
            active_container: None
        })
    };
}

impl LayoutTree {
    /// Gets the currently active container.
    pub fn get_active_container(&self) -> Option<&Container> {
        self.active_container.and_then(|ix| self.tree.get(ix))
    }

    /// Gets the currently active container.
    pub fn get_active_container_mut(&mut self) -> Option<&mut Container> {
        self.active_container.and_then(move |ix| self.tree.get_mut(ix))
    }

    /// Gets the index of the currently active output
    fn active_ix_of(&self, ctype: ContainerType) -> Option<NodeIndex> {
        if let Some(ix) = self.active_container {
            if self.tree[ix].get_type() == ctype {
                return Some(ix)
            }
            return self.active_container.and_then(|active|
                            self.tree.ancestor_of_type(active, ctype))
        }
        return None
    }

    fn active_of(&self, ctype: ContainerType) -> Option<&Container> {
        self.active_ix_of(ctype).and_then(|ix| self.tree.get(ix))
    }

    fn active_of_mut(&mut self, ctype: ContainerType) -> Option<&mut Container> {
        self.active_ix_of(ctype).and_then(move |ix| self.tree.get_mut(ix))
    }

    /// Gets the WlcOutput the active container is located on
    pub fn get_active_output(&self) -> Option<&Container> {
        self.active_of(ContainerType::Output)
    }

    /// Gets the WlcOutput the active container is located on
    pub fn get_active_output_mut(&mut self) -> Option<&mut Container> {
        self.active_of_mut(ContainerType::Output)
    }

    /// Gets the workspace the active container is located on
    pub fn get_active_workspace(&self) -> Option<&Container> {
        self.active_of(ContainerType::Workspace)
    }

    /// Gets the workspace the active container is located on
    pub fn get_active_workspace_mut(&mut self) -> Option<&mut Container> {
        self.active_of_mut(ContainerType::Workspace)
    }

    /// Gets the index of the workspace of this name
    ///
    /// TODO will search all outputs, probably should be more directed
    fn workspace_ix_by_name(&self, name: &str) -> Option<NodeIndex> {
        for output in self.tree.children_of(self.tree.root_ix()) {
            for workspace in self.tree.children_of(output) {
                if self.tree[workspace].get_name()
                    .expect("workspace_by_name: bad tree structure") == name {
                    return Some(workspace)
                }
            }
        }
        return None
    }

    /// Gets a workspace by name or creates it
    fn get_or_make_workspace(&mut self, name: &str) -> NodeIndex {
        let active_index = self.active_ix_of(ContainerType::Output).expect("get_or_make_wksp: Couldn't get output");
        self.workspace_ix_by_name(name).unwrap_or_else(|| {
            let root_ix = self.init_workspace(name.to_string(), active_index);
            self.tree.parent_of(root_ix)
                .expect("Workspace was not properly initialized with a root container")
        })
    }

    /// Gets a workspace by name
    pub fn get_workspace_by_name(&self, name: &str) -> Option<&Container> {
        self.workspace_ix_by_name(name).and_then(|ix| self.tree.get(ix))
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
        container_ix
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
        if let Some(worksp_ix) = self.active_ix_of(ContainerType::Workspace) {
            trace!("Adding {:?} to workspace {:?}", view, worksp_ix);
            let container_ix = self.tree.children_of(worksp_ix)[0];
            let view_ix = self.tree.add_child(container_ix,
                                                   Container::new_view(view));
            self.active_container = Some(view_ix);
            self.validate();
        }
    }

    //// Remove a view container from the tree
    pub fn remove_view(&mut self, view: &WlcView) {
        let mut maybe_parent = None;
        if let Some(view_ix) = self.tree.descendant_with_handle(self.tree.root_ix(), view) {
            // Check to not disrupt
            if self.active_container.map(|c| c == view_ix).unwrap_or(false) {
                let parent = self.tree.ancestor_of_type(view_ix, ContainerType::Container)
                    .expect("remove_view: workspace had no other containers");
                maybe_parent = Some(parent);
            }
            // Remove the view from the tree
            self.tree.remove(view_ix);
        }
        // Update the active container if needed
        if let Some(parent) = maybe_parent {
            self.focus_on_next_container(parent);
        }
        self.validate();
    }

    /// Updates the current active container to be the next container or view
    /// to focus on after the previous view/container was moved/removed.
    ///
    /// A new view will tried to be set, starting with the siblings of the
    /// removed node. If a view cannot be found there, it starts climbing the
    /// tree until either a view is found or the workspace is (in which case
    /// it set the active container to the root container of the workspace)
    ///
    /// Parent should be the parent of the node that was destroyed.
    /// Note that the node should be destroyed already, otherwise this algorithm
    /// will simply relocate the node to be destroyed and set it to be the active
    /// container.
    fn focus_on_next_container(&mut self, mut parent_ix: NodeIndex) {
        while self.tree.node_type(parent_ix)
            .expect("focus_next: unable to iterate") != ContainerType::Workspace {
            if let Some(view_ix) = self.tree.descendant_of_type(parent_ix,
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
        let mut container_ix = self.tree.children_of(parent_ix)[0];
        if let Some(view_ix) = self.tree.children_of(container_ix).get(0) {
            container_ix = *view_ix;
        }
        trace!("Active container set to container {:?}", container_ix);
        self.active_container = Some(container_ix);

        // Update focus to new container
        self.get_active_container().map(|con| match *con {
            Container::View { ref handle, .. } => handle.focus(),
            Container::Container { .. } => WlcView::root().focus(),
            _ => panic!("Active container not view or container!")
        });

        self.validate();
    }

    /// Switch to the specified workspace
    pub fn switch_to_workspace(&mut self, name: &str) {
        if self.active_container.is_none() {
            warn!("No active container, cannot switch");
            return;
        }
        // Set old workspace to be invisible
        if let Some(old_worksp_ix) = self.active_ix_of(ContainerType::Workspace) {
            trace!("Switching to workspace {}", name);
            self.tree.set_family_visible(old_worksp_ix, false);
        } else {
            warn!("Could not find old workspace, could not set invisible");
            return;
        }
        // Get the new workspace, or create one if it doesn't work
        let workspace_ix = self.get_or_make_workspace(name);
        // Set the new one to visible
        self.tree.set_family_visible(workspace_ix, true);
        self.focus_on_next_container(workspace_ix);
        self.validate();
    }

    /// Moves the current active container to a new workspace
    pub fn send_active_to_workspace(&mut self, name: &str) {
        // Ensure focus
        if self.active_container.is_none() {
            return;
        }

        let active_ix = self.active_container.expect("Asserted unwrap");
        let curr_work_ix = self.active_ix_of(ContainerType::Workspace)
            .expect("send_active: Not currently in a workspace!");
        self.tree.set_family_visible(curr_work_ix, false);
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

        // Get active
        if cfg!(debug_assertions) {
            let work_children = self.tree.children_of(curr_work_ix);
            assert!(work_children.len() != 0, "Workspace has no children");
            assert!(match self.tree[active_ix].get_type() {
                ContainerType::Container|ContainerType::View => true,
                _ => false
            }, "Invalid workspace switch type!");
        }
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
        if let Some(parent) = maybe_active_parent {
            let ctype = self.tree.node_type(parent).unwrap_or(ContainerType::Root);
            if ctype == ContainerType::Container {
                self.focus_on_next_container(parent);
            }
            else {
                trace!("Send to container invalidated a NodeIndex: {:?} to {:?}",
                parent, ctype);
            }
        }
        else {
            self.focus_on_next_container(curr_work_ix);
        }

        self.validate();
    }

    /// Validates the tree
    #[cfg(debug_assertions)]
    fn validate(&self) {
        warn!("Validating the tree");

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

        // For each view, ensure it's a node in the tree
        for output_ix in self.tree.children_of(self.tree.root_ix()) {
            let output = match self.tree[output_ix]
                .get_handle().expect("Child of root had no output") {
                    Handle::Output(output) => output,
                    _ => panic!("Output container had no output")
                };
            for view in output.get_views() {
                if self.tree.descendant_with_handle(output_ix, &view).is_none() {
                    error!("View handle {:?} could not be found for {:?}",
                           view, output);
                    trace!("The tree: {:#?}", self);
                    panic!()
                }
            }
        }

        // Ensure active container is in tree and of right type
        if let Some(active_ix) = self.active_container {
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

        // Ensure workspace have at least one child
        for output_ix in self.tree.children_of(self.tree.root_ix()) {
            for workspace_ix in self.tree.children_of(output_ix) {
                if self.tree.children_of(workspace_ix).len() == 0 {
                    error!("Workspace {:#?} has no children",
                           self.tree[workspace_ix]);
                    trace!("The tree: {:#?}", self);
                    panic!()
                }
            }
        }
    }

    #[cfg(not(debug_assertions))]
    fn validate_tree() {}

}

/// Attempts to lock the tree. If the Result is Err, then a thread that
/// previously had the lock panicked and potentially left the tree in a bad state
pub fn try_lock_tree() -> TreeResult {
    trace!("Locking the tree!");
    TREE.try_lock()
}

