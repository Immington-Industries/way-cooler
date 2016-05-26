//! Main module to handle the layout.
//! This is where the i3-specific code is.

use std::sync::{Mutex, MutexGuard, TryLockError};
use std::ptr;

use super::container::{Container, Handle, ContainerType};
use super::super::rustwlc::{WlcView, WlcOutput, Geometry, Point};

use petgraph::graph::{DefIndex, Graph, Node, NodeIndex, Neighbors};
use petgraph::EdgeDirection;

pub type TreeErr = TryLockError<MutexGuard<'static, Tree>>;
pub type TreeResult = Result<MutexGuard<'static, Tree>, TreeErr>;

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

/// A Tree of Nodes.
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
pub struct Tree {
    root: Graph<Container, ()>,
    active_container: Option<NodeIndex<Container>>,
}

unsafe impl Send for Tree {}

lazy_static! {
    static ref TREE: Mutex<Tree> = {
        let tree = Tree {
            root: Graph::new(),
            active_container: None,
        };
        tree.root.add_node(Container::new_root());
        Mutex::new(tree)
    };
}

impl Tree {

    /// Moves the current active container to a new workspace
    pub fn move_container_to_workspace(&mut self, name: &str) {
        // Ensure we are focused on something
        if self.get_active_container().is_none() {
            return;
        }
        if let Some(workspace) = self.get_active_workspace() {
            // Ensure we aren't trying to move nothing
            if workspace.get_children().len() == 1 {
                if workspace.get_children()[0].get_children().len() == 0 {
                    warn!("Tried to move a container not made by the user");
                    return;
                }
            }
            // Ensure we are moving to a new workspace
            if workspace.get_val().get_name().unwrap() == name {
                warn!("Tried to switch to already current workspace");
                return;
            }
        }
        // Ensure get_active_container is giving us a view or a container
        match self.get_active_container().unwrap().get_val().get_type() {
            ContainerType::Container | ContainerType::View => {},
            _ => {
                warn!("Tried to switch workspace on a non-view/container");
                return
            }
        }
        // If workspace doesn't exist, add it
        if self.get_workspace_by_name(name).is_none() {
            self.add_workspace(name.to_string());
        }
        info!("Moving container {:?} to workspace {}", self.get_active_container(), name);
        let moved_container: Node<Container>;
        let parent: *const Node<Container>;
        // Move the container out (and set it to be invisible),
        // get the moved_container to be placed into new workspace
        // and parent so that we can get the new active container on this workspace
        {
            let mut_container = self.get_active_container_mut().unwrap();
            parent = mut_container.get_parent().unwrap() as *const Node;
            moved_container = mut_container.remove_from_parent()
                .expect("Could not remove container, was not part of tree");
            mut_container.set_visibility(false);
            trace!("Removed container {:?}", moved_container);
        }
        // Put container into the new workspace
        if let Some(workspace) = self.get_workspace_by_name_mut(name) {
            let new_parent_container = &mut workspace.get_children_mut()[0];
            new_parent_container.add_child(moved_container)
                .expect("Could not moved container to other a workspace");
            trace!("Added previously removed container to {:?} in workspace {}",
                   new_parent_container,
                   name);
        }
        unsafe { self.update_removed_active_container(&*parent); }
        // Update the focus to the new active container
        match *self.get_active_container()
            .and_then(|container| Some(container.get_val())).unwrap() {
                Container::View { ref handle, ..} => handle.focus(),
                Container::Container { .. } => WlcView::root().focus(),
                _ => panic!("Active Container was not a view or container")
            }
        self.validate_tree();
    }

    /// Switch to the workspace with the give name
    pub fn switch_workspace(&mut self, name: &str ) {
        if self.active_container.is_null() {
            warn!("No current active container, cannot switch workspace");
            return;
        }
        // Set the old workspace to be invisible
        if let Some(old_workspace) = self.get_active_workspace_mut() {
            trace!("Switching to workspace {}", name);
            old_workspace.set_visibility(false);
        } else {
            warn!("Could not find old workspace, could not set invisible");
            return;
        }
        // If the workspace we are switching to doesn't exist, add it
        if let Some(_) = self.get_workspace_by_name(name) {
            trace!("Found workspace {}", name);
        } else {
            trace!("Adding workspace {}", name);
            self.add_workspace(name.to_string());
        }
        let new_active_container: *const Node;
        {
            let view: WlcView;
            /* Get the new workspace, make it visible */
            {
                let new_current_workspace = self.get_workspace_by_name_mut(name)
                    .expect(ERR_BAD_TREE);
                new_current_workspace.set_visibility(true);
                /* Set the first view to be focused, so the screen refreshes itself */
                view = Node::focus_first_view(new_current_workspace);
            }
            if view == WlcView::root() {
                new_active_container = &self.get_workspace_by_name(name)
                    .expect(ERR_BAD_TREE).get_children()[0];
            } else {
                new_active_container = self.root.find_view_by_handle(&view)
                    .expect("Could not find view we just found");
            }
        }
        // Update the tree's pointer to the currently focused container
        unsafe { self.set_active_container(&*new_active_container).unwrap(); }
        self.validate_tree();
    }

    /// Sets the currently active container.
    ///
    /// This function will return an Err if the container is not either a
    /// View or a Container
    fn set_active_container(&mut self, node: &Node<Container>) -> Result<(), ()> {
        match node.get_val().get_type() {
            ContainerType::View | ContainerType::Container => {
                self.active_container = node as *const Node;
            },
            _ => {
                error!("Tried to set {:?} as active container", node);
                return Err(());
            }
        }
        self.validate_tree();
        Ok(())
    }

    /// Returns the currently active container.
    ///
    /// If this returns a Node, the node contains either a View or a Container
    pub fn get_active_container(&self) -> Option<&Node<Container>> {
        if self.active_container.is_null() {
            None
        } else {
            unsafe {
                Some(&*self.active_container)
            }
        }
    }

    /// Returns the currently active container as mutable.
    fn get_active_container_mut(&mut self) -> Option<&mut Node<Container>> {
        if let Some(container) = self.get_active_container() {
            unsafe { Some(container.as_mut()) }
        } else {
            None
        }
    }


    /// Get the monitor (output) that the active container is located on
    pub fn get_active_output(&self) -> Option<&Node<Container>> {
        if let Some(node) = self.get_active_container() {
            if node.get_val().get_type() == ContainerType::Output {
                Some(node)
            } else {
                node.get_ancestor_of_type(ContainerType::Output)
            }
        } else {
            None
        }
    }

    /// Get the monitor (output) that the active container is located on
    /// as mutable
    fn get_active_output_mut(&mut self) -> Option<&mut Node<Container>> {
        if let Some(output) = self.get_active_output() {
            unsafe { Some(output.as_mut()) }
        } else {
            None
        }
    }

    /// Get the workspace that the active container is located on
    pub fn get_active_workspace(&self) -> Option<&Node<Container>> {
        if let Some(node) = self.get_active_container() {
            if node.get_val().get_type() == ContainerType::Workspace {
                return Some(node)
            }
            if let Some(workspace) = node.get_ancestor_of_type(ContainerType::Workspace) {
                return Some(workspace)
            }
        }
        self.validate_tree();
        return None
    }

    /// Get the workspace that the active container is located on mutably
    fn get_active_workspace_mut(&mut self) -> Option<&mut Node<Container>> {
        if let Some(workspace) = self.get_active_workspace() {
            unsafe { Some(workspace.as_mut())  }
        } else {
            None
        }
    }

    /// Find the workspace node that has the given name
    pub fn get_workspace_by_name(&self, name: &str) -> Option<&Node<Container>> {
        for child in self.root.get_children()[0].get_children() {
            if child.get_val().get_name().expect(ERR_BAD_TREE) != name {
                continue
            }
            return Some(child);
        }
        return None
    }

    /// Find the workspace node that has the given name as mut
    fn get_workspace_by_name_mut(&mut self, name: &str) -> Option<&mut Node<Container>> {
        if let Some(workspace) = self.get_workspace_by_name(name) {
            unsafe { Some(workspace.as_mut()) }
        } else {
            None
        }
    }

    /// Make a new output container with the given WlcOutput.
    ///
    /// A new workspace is automatically added to the output, to ensure
    /// consistency with the tree. By default, it sets this new workspace to
    /// be workspace "1". This will later change to be the first available
    /// workspace if using i3-style workspaces.
    #[allow(unused_assignments)]
    pub fn add_output(&mut self, wlc_output: WlcOutput) {
        trace!("Adding new output with WlcOutput: {:?}", wlc_output);
        let mut new_active_container: *const Node = ptr::null();
        match self.root.new_child(Container::new_output(wlc_output)) {
            Ok(output) => {
                new_active_container = Tree::init_workspace("1".to_string(), output).unwrap()
                    as *const Node;
            },
            Err(e) => {
                error!("Could not add output: {:?}", e);
                return;
            }
        }
        // If there is no active container, set it to this new one we just made
        if self.get_active_container().is_none() && new_active_container != ptr::null() {
            unsafe {self.set_active_container(&*new_active_container).unwrap(); }
        }
        self.validate_tree();
    }

    /// Make a new workspace container with the given name on the current active output.
    ///
    /// A new container is automatically added to the workspace, to ensure
    /// consistency with the tree. This container has no children, and should
    /// not be moved.
    pub fn add_workspace(&mut self, name: String) -> Option<&Node<Container>> {
        if let Some(output) = self.get_active_output_mut() {
            return Tree::init_workspace(name, output)
        } else {
            warn!("Could not get active output");
        }
        None
    }

    /// Initialize a workspace with the given name on some output.
    ///
    /// A new container is automatically added to the workspace, to ensure
    /// consistency with the tree. This container has no children, and should
    /// not be moved.
    fn init_workspace(name: String, output: &mut Node<Container>) -> Option<&Node<Container>> {
        let size = output.get_val().get_geometry()
            .expect("Output did not have a geometry").size;
        let workspace = Container::new_workspace(name.to_string(),
                                                    size.clone());
        match output.new_child(workspace) {
            Ok(workspace) => {
                trace!("Added workspace {:?}", workspace);
                let geometry = Geometry {
                    size: size,
                    origin: Point { x: 0, y: 0}
                };
                match workspace.new_child(Container::new_container(geometry)) {
                    Ok(container) => { return Some(container) },
                    Err(e) => error!("Could not add container to workspace: {:?}",e ),
                };
            },
            Err(e) => error!("Could not add workspace: {:?}", e),
        }
        None
    }

    /// Make a new view container with the given WlcView, and adds it to
    /// the active workspace.
    pub fn add_view(&mut self, wlc_view: WlcView) {
        let mut maybe_new_view: *const Node = ptr::null();
        if let Some(current_workspace) = self.get_active_workspace_mut() {
            trace!("Adding view {:?} to {:?}", wlc_view, current_workspace);
            let container = &mut current_workspace.get_children_mut()[0];
            match container.new_child(Container::new_view(wlc_view)) {
                Ok(view_node) => {
                    maybe_new_view = view_node as *const Node;
                },
                Err(e) => {
                    error!("Could not add view to current workspace: {:?}", e);
                    return;
                }
            }
        };
        if ! maybe_new_view.is_null() {
            unsafe { self.set_active_container(&*maybe_new_view).unwrap(); }
        }
        self.validate_tree();
    }

    /// Remove the view container with the given view
    pub fn remove_view(&mut self, wlc_view: WlcView) {
        let mut maybe_parent: *const Node = ptr::null();
        if let Some(view) = self.root.find_view_by_handle(&wlc_view) {
            // Ensure that we are not invalidating the active_container pointer
            if (view as *const Node) == self.active_container {
                // Since we are just removing a single view,
                // keep going up to ancestors to find a view
                // not being invalidated
                let parent = view.get_ancestor_of_type(ContainerType::Container).unwrap();
                maybe_parent = parent as *const Node;
                // Remove node before we search, so that we can't accidentally
                // re-select it when traversing tree
                unsafe { view.as_mut().remove_from_parent(); }
            } else {
                // Remove node
                unsafe { view.as_mut().remove_from_parent(); }
            }
        }
        if ! maybe_parent.is_null() {
            unsafe { self.update_removed_active_container(&*maybe_parent); }
        }
        self.validate_tree();
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
    fn update_removed_active_container(&mut self, mut parent: &Node<Container>) {
        while parent.get_container_type() != ContainerType::Workspace {
            if let Some(view) = parent.get_descendant_of_type(ContainerType::View) {
                match view.get_val().get_handle().expect("View had no handle") {
                    Handle::View(view) => view.focus(),
                    _ => panic!("View had an output handle")
                }
                unsafe {self.set_active_container(&*(view as *const Node)).unwrap()};
                trace!("Active container set to view {:?}", view);
                return;
            }
            parent = parent.get_ancestor_of_type(ContainerType::Container)
                .unwrap_or_else(|| {
                    parent.get_ancestor_of_type(ContainerType::Workspace)
                        .expect("Container was not part of a workspace")
                });
        }
        // parent is a workspace
        let container = &parent.get_children()[0];
        trace!("Active container set to container {:?}", container);
        unsafe { self.set_active_container(&*(container as *const Node)).unwrap() };
        self.validate_tree();
    }

    // Validates the invariants of the tree
    fn validate_tree(&self) {
        warn!("Validating the tree");
        // Ensure the each child node points to its parent
        fn validate_node_connections(this: &Tree, parent: &NodeIndex) {
            for child in this.get_children(parent) {
                let child_parent = this.get_parent(child);
                if child_parent != *parent {
                    error!("Child {:#?} has parent {:#?}, expected {:#?}",
                           child, child_parent, parent);
                    panic!();
                }
                validate_node_connections(this, &child);
            }
        }
        validate_node_connections(self, &self.root);
        // For each view, ensure that it's a node in the tree
        for output in self.root.get_children() {
            let view_list = match output.get_val().get_handle().expect("Output had no handle") {
                Handle::Output(output) => output.get_views(),
                _ => panic!("Output container did not have an WlcOutput")
            };
            for view in view_list {
                if output.find_view_by_handle(&view).is_none() {
                    error!("View handle {:#?} could not be found on output {:#?}", view, output);
                    panic!();
                }
            }
        }
        // Ensure the active container is in the tree and is of the right type
        if let Some(active_container) = self.get_active_container() {
            match active_container.get_val().get_type() {
                ContainerType::View | ContainerType::Container => {},
                _ => panic!("Active container was not a View or a Container")
            }
            if active_container.get_ancestor_of_type(ContainerType::Root).is_none() {
                error!("Active container is currently: {:#?}", active_container);
                error!("Active container is not part of tree");
                panic!();
            }
        }
        // Ensure that workspaces that exist at least have one child
        for output in self.get_children(NodeIndex::new(0)) {
            for workspace in self.get_children(output) {
                if self.get_children(workspace).collect().len() == 0 {
                    error!("Workspace {:#?} is invalid, expected at least one child, had none", workspace);
                }
            }
        }
    }

    /* NOTE: Put these functions in the wrapper */

    /// Gets the children of the Tree Node
    fn get_children(&self, node_index: NodeIndex) -> Vec<NodeIndex> {
        self.root.neighbors_directed(node_index, EdgeDirection::Outgoing).collect()
    }

    /// Gets the parent (only directed graph into) for this node
    fn get_parent(&self, node_index: NodeIndex) -> NodeIndex {
        let parent_edge = self.root.first_edge(node_index, EdgeDirection::Incoming)
                           .expect("Did not have a parent");
        self.root.edge_endpoints(parent_edge).unwrap().1
    }
}

/// Attempts to lock the tree. If the Result is Err, then a thread that
/// previously had the lock panicked and potentially left the tree in a bad state
pub fn try_lock_tree() -> TreeResult {
    trace!("Locking the tree!");
    TREE.try_lock()
}

