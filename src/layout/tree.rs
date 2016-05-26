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
        self.active_container.and_then(|ix| self.tree[ix])
    }

    /// Gets the currently active container.
    pub fn get_active_container_mut(&mut self) -> Option<&mut Container> {
        self.active_container.and_then(|ix| self.tree.get_mut(ix))
    }

    /// Gets the index of the currently active output
    fn active_ix_of(&self, ctype: ContainerType) -> Option<NodeIndex> {
        if let Some(ix) = self.active_container {
            if self[ix].get_type() == ctype {
                return Some(ix)
            }
            return self.get_ancestor_of_type(
                self.active_container, ctype)
        }
        return None
    }

    fn get_active_of(&self, ctype: ContainerType) -> Option<&Container> {
        self.active_ix_of(ctype).and_then(|ix| self.tree[ix])
    }

    fn get_active_of_mut(&self, ctype: ContainerType) -> Option<&Container> {
        self.active_ix_of(ctype).and_then(|ix| self.tree.get_mut(ix))
    }

    /// Gets the WlcOutput the active container is located on
    pub fn get_active_output(&self) -> Option<&Container> {
        self.get_active_of(ContainerType::Output)
    }

    /// Gets the WlcOutput the active container is located on
    pub fn get_active_output_mut(&mut self) -> Option<&mut Container> {
        self.get_active_of_mut(ContainerType::Output)
    }

    /// Gets the workspace the active container is located on
    pub fn get_active_workspace(&self) -> Option<&Container> {
        self.get_active_of(ContainerType::Workspace)
    }

    /// Gets the workspace the active container is located on
    pub fn get_active_workspace_mut(&mut self) -> Option<&mut Container> {
        self.get_active_of(ContainerType::Workspace)
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

    /// Gets a workspace by name
    pub fn get_workspace_by_name(&self, name: &str) -> Option<&Container> {
        self.workspace_ix_by_name.map(|ix| self.tree[ix])
    }

    /// Initializes a workspace and gets the index of the root container
    fn init_workspace(&mut self, name: String, output_ix: NodeIndex)
                      -> NodeIndex {
        let size = self.tree[output_ix]
            .expect("init_workspace: invalid output").get_geometry()
            .expect("init_workspace: no geometry for output").size;
        let worksp = Container::new_workspace(name.to_string(), size.clone());

        let (_, worksp_ix) = self.tree.add_child(output_ix, worksp);
        trace!("Added workspace {:?}", worksp);
        let geometry = Geometry {
            size: size, origin: Point { x: 0, y: 0 }
        };
        let (_, container_ix) = self.tree.add_child(worksp_ix,
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
        let (_, output_ix) = self.tree.add_child(self.tree.root_ix(),
                                          Container::new_output(output));
        self.active_container = self.init_workspace("1".to_string(), output_ix);
        self.validate();
    }

    /// Make a new workspace container with the given name on the current active output.
    ///
    /// A new container is automatically added to the workspace, to ensure
    /// consistency with the tree. This container has no children, and should
    /// not be moved.
    fn new_workspace_ix(&mut self, name: String) -> Option<NodeIndex> {
        if let Some(output_ix) = self.active_output_ix() {
            return self.init_workspace(name, output_ix)
        } else {
            warn!("Could not get active output");
        }
        None
    }

    /// Add a new view container with the given WlcView to the active container
    pub fn add_view(&mut self, view: WlcView) {
        if let Some(worksp_ix) = self.active_workspace_ix() {
            trace!("Adding {:?} to workspace {:?}", view, worksp_ix);
            let container_ix = self.tree.children_of(worksp_ix).first()
                .expect("add_view: current workspace had no container");
            let (_, view_ix) = self.tree.add_child(container_ix,
                                                   Container::new_view(view));
            self.active_container = Some(view_ix);
            self.validate();
        }
    }

    //// Remove a view container from the tree
    pub fn remove_view(&mut self, view: &WlcView) {
        if let Some(view_ix) = self.tree.find_view_by_handle(self.tree.root_ix(), view) {
            let mut maybe_parent = None;
            // Check to not disrupt
            if view_ix == self.active_container {
                let parent = self.tree.ancestor_of_type(view_ix, ContainerType::Container)
                    .expect("remove_view: workspace had no other containers");
                maybe_parent = Some(parent);
            }
            // Remove the view from the tree
            self.tree.remove(view_ix);
        }
        // Update the active container if needed
        if let Some(parent) = maybe_parent {
            self.update_removed_active_cotnainer(parent);
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
    fn update_removed_active_container(&mut self, mut parent_ix: NodeIndex) {
        while self.tree.node_type(parent_ix) != ContainerType::Workspace {
            if let Some(view_ix) = self.tree.descendant_of_type(parent,
                                                           ContainerType::View) {
                match self.tree[view_ix].expect("already found ix")
                                    .get_handle().expect("view had no handle") {
                    Handle::View(view) => view.focus(),
                    _ => panic!("View had an output handle")
                }
                trace!("Active container set to view at {}", view_ix);
                self.active_container = Some(view_ix);
                return;
            }
            parent_ix = self.tree.ancestor_of_type(parent_ix,
                                                   ContainerType::Container)
                .unwrap_or_else(|| {
                    self.tree.ancestor_of_type(ContainerType::Workspace)
                        .expect("Container was not part of a workspace")
                });
        }
        // If this is reached, parent is workspace
        let container_ix = self.tree.children_of(parent_ix).first()
            .expect("Workspace had no children");
        trace!("Active container set to container {}", container_ix);
        self.active_container = Some(container_ix);
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
            //self.tree.set_family_visible(old_worksp_ix, false);
            self.tree.get_mut(old_worksp_ix).expect("Asserted unwrap")
                .set_visibility(false);
        } else {
            warn!("Could not find old workspace, could not set invisible");
            return;
        }
        // If the workspace doesn't exist add it
        if let Some(_) = self.workspace_ix_by_name(name) {
            trace!("Found workspace {}", name);
        } else {
            trace!("Adding workspace {}", name);
            self.new_workspace_ix(name.to_string())
                .expect("unable to create workspace");
        }
        // Update new workspace visibility
        let workspace_ix = self.workspace_ix_by_name(name)
            .expect("switch_to_workspace: unable to create workspace?");
        self.tree.get_mut(workspace_ix).expect("Asserted unwrap")
            .set_visibility(true);
        // Get the first view to be focused, so the screen updates
        let view = self.tree.first_view_to_focus(workspace_ix);
        if view.is_root() {
            self.active_container = self.tree.children_of(
                self.workspace_ix_by_name(name).expect("Just made workspace"))
                .first();
        } else {
            self.active_container = Some(self.find_view_ix_by_handle(&view)
                .expect("Could not find view we just found"));
        }
        self.validate();
    }

    /// Moves the current active container to a new workspace
    pub fn send_active_to_workspace(&mut self, name: &str) {
        // Ensure focus
        if self.active_container.is_none() {
            return;
        }
        let active_ix = self.get_active_container.expect("Asserted unwrap");
        // Get active
        if let Some(worksp_ix) = self.get_active_workspace() {
            if cfg!(debug_asserts) {
                let workspace = self.tree.get(worksp_ix);
                assert!(self.tree.children_of(worksp_ix).count() > 0,
                        "Workspace child has no output");
                assert!(self.tree.children_of(worksp_ix).first()
                        .expect("send_active_to_workspace: debug asserts")
                        .children().count() > 0,
                        "Move container not made by user");
                assert!(match self.tree[active].get_type() {
                    ContainerType::Container|ContainerType::View => true,
                    _ => false
                }, "Invalid workspace switch type!");
            }

            // If workspace doesn't exist, add it
            if self.get_workspace_by_name(name).is_none() {
                self.add_workspace(name.to_string());
            }
            let maybe_active_parent = self.tree.parent_of(active);
            // Move the container
            info!("Moving container {} to workspace {}", active, name);
            self.tree.move_node(active, worksp_ix); // This existed before petgraph

            // Update the active container
            if let Some(parent) = maybe_active_parent {
                self.focus_on_next_container(parent);
            }

            // Update focus to new container
            // TODO make this its own method
            match *self.get_active_container().map(Container::get_val) {
                Container::View { ref handle, .. } => handle.focus(),
                Container::Container { .. } => WlcView::root().focus(),
                _ => panic!("Active container not view or container!")
            }
        }
        self.validate();
    }

    /// Validates the tree
    #[cfg(debug_debug_assertions)]
    pub fn validate(&self) {
        warn!("Validating the tree");

        // Recursive method to ensure child/parent nodes are connected
        fn validate_node_connectons(this: &Tree, parent_ix: NodeIndex) {
            for child_ix in self.tree.children_of(parent_ix) {
                let child_parent = self.tree.parent_of(child_ix)
                    .expect("connections: Child did not point to parent!");
                if child_parent != parent_ix {
                    error!("Child at {} has parent {}, expected {}",
                           child_ix, child_parent, parent_ix);
                    trace!("The tree: {:#?}", this);
                    panic!()
                }
                validate_node_connections(this, child_ix);
            }
        }

        validate_node_connectons(self, self.tree.root_ix());

        // For each view, ensure it's a node in the tree
        for output_ix in self.tree.chidren_of(self.tree.root_ix()) {
            let output = match self.tree[output_ix]
                .expect("Couldn't find output listed from tree")
                .get_handle().expect("Child of root had no output") {
                    Handle::Output(output) => output,
                    _ => panic!("Output container had no output")
                };
            for view in output.get_views() {
                if self.tree.find_view_by_handle(output_ix).is_none() {
                    error!("View handle {:?} could not be found for {:?}",
                           view, output);
                    trace!("The tree: {#:?}", self);
                    panic!()
                }
            }
        }

        // Ensure active container is in tree and of right type
        if let Some(active_ix) = self.active_container {
            let active = self.get_active_conttainer()
                .expect("active_container points to invalid node");
            match active.get_type() {
                ContainerType::View | ContainerType::Container => {},
                _ => panic!("Active container was not view or container")
            }
            // Check active container in tree
            if self.tree.ancestor_of_type(active_ix, ContainerType::Root).is_none() {
                error!("Active container @ {} is not part of tree!", active_ix);
                error!("Active container is {:?}", active);
                trace!("The tree: {#:?}", self);
                panic!()
            }
        }

        // Ensure workspace have at least one child
        for output_ix in self.tree.children_of(self.tree.root_ix()) {
            for workspace_ix in self.tree.children_of(output_ix) {
                if self.tree.children_of(workspace_ix).next().is_none() {
                    error!("Workspace {:#?} has no children",
                           self.tree[workspace_ix]);
                    trace!("The tree: {#:?}", self);
                    panic!()
                }
            }
        }
    }

    #[cfg(not(debug_assertions))]
    #[inline]
    fn validate_tree() {}

}

/// Attempts to lock the tree. If the Result is Err, then a thread that
/// previously had the lock panicked and potentially left the tree in a bad state
pub fn try_lock_tree() -> TreeResult {
    trace!("Locking the tree!");
    TREE.try_lock()
}

