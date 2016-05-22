//! Main module to handle the layout.
//! This is where the i3-specific code is.

use std::sync::{Mutex, MutexGuard, TryLockError};
use std::ptr;

use super::container::{Container, Handle, ContainerType};
use super::node::{Node};
use super::super::rustwlc::{WlcView, WlcOutput, Geometry, Point, ResizeEdge};


pub type TreeErr = TryLockError<MutexGuard<'static, Tree>>;
pub type TreeResult = Result<MutexGuard<'static, Tree>, TreeErr>;

const ERR_BAD_TREE: &'static str = "Layout tree was in an invalid configuration";

// NOTE Make an actual image of this, embed in documentation
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
    root: Node,
    active_container: *const Node,
}

unsafe impl Send for Tree {}

lazy_static! {
    static ref TREE: Mutex<Tree> = {
        Mutex::new(Tree{
            root: Node::new(Container::new_root()),
            active_container: ptr::null(),
        })
    };
}

impl Tree {

    /// Moves the current active container to a new workspace
    pub fn move_container_to_workspace(&mut self, name: &str) {
        if self.get_active_container().is_none() {
            return;
        }
        if let Some(workspace) = self.get_active_workspace() {
            if workspace.get_children().len() == 1 {
                if workspace.get_children()[0].get_children().len() == 0 {
                    warn!("Tried to move a container not made by the user");
                    return;
                }
            }
            if workspace.get_val().get_name().unwrap() == name {
                warn!("Tried to switch to already current workspace");
                return;
            }
        }
        match self.get_active_container().unwrap().get_val().get_type() {
            //ContainerType::Container => {},
            ContainerType::View => {},
            _ => {
                warn!("Tried to switch workspace on a non-view/container");
                return
            }
        }
        if self.get_workspace_by_name(name).is_none() {
            self.add_workspace(name.to_string());
        }
        info!("Moving container {:?} to workspace {}", self.get_active_container(), name);
        {
            let container = self.get_active_container().unwrap();

            // Make view invisible, switch focus
            {
                let parent = container.get_parent().expect("Had no parent");
                // There is another view we can focus on
                if parent.get_children().len() > 1 {
                    trace!("Attempting to focus on a view");
                    let mut children_pool: Vec<&Node>  = parent.get_children().iter()
                        .filter(|child| *child != container).collect();
                    while children_pool.len() > 0 {
                        let child = children_pool.pop().unwrap();
                        if child.get_val().get_type() == ContainerType::View {
                            trace!("Focused on a view");
                            match child.get_val().get_handle().unwrap() {
                                Handle::View(view) => view.focus(),
                                _ => panic!("Expect WlcView, got WlcOutput"),
                            }
                            break;
                        } else {
                            let more_children: Vec<&Node> = child.get_children().iter()
                                .collect();
                            children_pool.extend(more_children);
                        }
                    }
                } else {
                    trace!("Focusing on root, because no other views");
                    WlcView::root().focus();
                }
            }
        }
        let moved_container: Node;
        let parent: *const Node;
        // Move the container out
        {
            let mut_container = self.get_active_container_mut().unwrap();
            parent = mut_container.get_parent().unwrap() as *const Node;
            moved_container = mut_container.remove_from_parent()
                .expect("Could not remove container, was not part of tree");
            mut_container.set_visibility(false);
            trace!("Removed container {:?}", moved_container);
        }
        unsafe { self.update_removed_active_container(&*parent); }
        // Put into the new workspace
        if let Some(workspace) = self.get_workspace_by_name_mut(name) {
            let new_parent_container = &mut workspace.get_children_mut()[0];
            new_parent_container.add_child(moved_container)
                .expect("Could not moved container to other a workspace");
            trace!("Added previously removed container to {:?} in workspace {}",
                    new_parent_container,
                name);
        }
    }

    /// Switch to the workspace with the give name
    pub fn switch_workspace(&mut self, name: &str ) {
        if self.active_container.is_null() {
            warn!("No current active container, cannot switch workspace");
            return;
        }
        if let Some(old_workspace) = self.get_active_workspace_mut() {
            trace!("Switching to workspace {}", name);
            old_workspace.set_visibility(false);
        } else {
            warn!("Could not find old workspace, could not set invisible");
            return;
        }
        if let Some(_) = self.get_workspace_by_name(name) {
            trace!("Found workspace {}", name);
        } else {
            trace!("Adding workspace {}", name);
            self.add_workspace(name.to_string());
        }
        let new_active_container: *const Node;
        {
            if let Some(_) = self.get_workspace_by_name(name) {
                trace!("Found workspace {}", name);
            } else {
                trace!("Adding workspace {}", name);
                self.add_workspace(name.to_string());
            }
            let new_current_workspace = self.get_workspace_by_name_mut(name)
                .expect(ERR_BAD_TREE);
            new_current_workspace.set_visibility(true);
            /* Set the first view to be focused, so the screen refreshes itself */
            if new_current_workspace.get_children()[0].get_children().len() > 0 {
                trace!("Focusing view");
                let view_container = &new_current_workspace.get_children_mut()[0]
                    .get_children_mut()[0];
                match view_container.get_val().get_handle().expect(ERR_BAD_TREE) {
                    Handle::View(view) => view.focus(),
                    _ => {panic!("Expected view, got Wlc Output")},
                }
                new_active_container = view_container as *const Node;
            }
            /* If there is no view in the new workspace, just set the focused container
            to be the workspace's only container */
            else {
                let child_container = &new_current_workspace.get_children()[0];
                new_active_container = child_container as *const Node;
                WlcView::root().focus();
            }
        }
        // Update the tree's pointer to the currently focused container
        unsafe { self.set_active_container(&*new_active_container).unwrap(); }
    }


    /// Sets the currently active container.
    ///
    /// This function will return an Err if the container is not either a
    /// View or a Container
    pub fn set_active_container(&mut self, node: &Node) -> Result<(), ()> {
        match node.get_val().get_type() {
            ContainerType::View => {},
            ContainerType::Container => {},
            _ => {
                error!("Tried to set {:?} as active container", node);
                return Err(());
            }
        }
        self.active_container = node as *const Node;
        Ok(())
    }

    /// Returns the currently active container.
    ///
    /// Note that this might be at certain times this might simply be
    /// an Output or a Workspace to ensure other methods work.
    ///
    /// NOTE This should change because a nice invariant to have would
    /// be that `get_active_container` only returns either a container
    /// or a view, since those are the only nodes that can be "active".
    pub fn get_active_container(&self) -> Option<&Node> {
        if self.active_container.is_null() {
            None
        } else {
            unsafe {
                Some(&*self.active_container)
            }
        }
    }

    /// Returns the currently active container as mutable.
    pub fn get_active_container_mut(&mut self) -> Option<&mut Node> {
        if let Some(container) = self.get_active_container() {
            unsafe { Some(container.as_mut()) }
        } else {
            None
        }
    }


    /// Get the monitor (output) that the active container is located on
    pub fn get_active_output(&self) -> Option<&Node> {
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
    pub fn get_active_output_mut(&mut self) -> Option<&mut Node> {
        if let Some(output) = self.get_active_output() {
            unsafe { Some(output.as_mut()) }
        } else {
            None
        }
    }

    /// Get the workspace that the active container is located on
    pub fn get_active_workspace(&self) -> Option<&Node> {
        if let Some(node) = self.get_active_container() {
            if node.get_val().get_type() == ContainerType::Workspace {
                return Some(node)
            }
            if let Some(workspace) = node.get_ancestor_of_type(ContainerType::Workspace) {
                return Some(workspace)
            }
        }
        return None
    }

    /// Get the workspace that the active container is located on mutably
    pub fn get_active_workspace_mut(&mut self) -> Option<&mut Node> {
        if let Some(workspace) = self.get_active_workspace() {
            unsafe { Some(workspace.as_mut())  }
        } else {
            None
        }
    }

    /// Find the workspace node that has the given name
    pub fn get_workspace_by_name(&self, name: &str) -> Option<&Node> {
        for child in self.root.get_children()[0].get_children() {
            if child.get_val().get_name().expect(ERR_BAD_TREE) != name {
                continue
            }
            return Some(child);
        }
        return None
    }

    /// Find the workspace node that has the given name as mut
    pub fn get_workspace_by_name_mut(&mut self, name: &str) -> Option<&mut Node> {
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
        // NOTE Should probably not be "1", should be the next unclaimed number
        if self.get_active_container().is_none() && new_active_container != ptr::null() {
            unsafe {self.set_active_container(&*new_active_container).unwrap(); }
        }
    }

    /// Make a new workspace container with the given name on the current active output.
    ///
    /// A new container is automatically added to the workspace, to ensure
    /// consistency with the tree. This container has no children, and should
    /// not be moved.
    ///
    /// NOTE Consider changing it to be a different type, something that will not and can
    /// not be moved? Another container type to check against then?
    pub fn add_workspace(&mut self, name: String) -> Option<&Node> {
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
    fn init_workspace<'a>(name: String, output: &'a mut Node) -> Option<&'a Node> {
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
                    // New active container should be this container of the new workspace
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
    }

    /// Remove the view container with the given view
    ///
    /// If this is the only view of that container, and that container
    /// is not the only one in the workspace, then that container is also
    /// removed.
    ///
    /// NOTE Implement \^\^\^
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
    fn update_removed_active_container(&mut self, mut parent: &Node) {
        let mut active_container_set = false;
        while parent.get_container_type() != ContainerType::Workspace {
            if let Some(view) = parent.get_descendant_of_type(ContainerType::View) {
                match view.get_val().get_handle().expect("View had no handle") {
                    Handle::View(view) => view.focus(),
                    _ => panic!("View had an output handle")
                }
                unsafe {self.set_active_container(&*(view as *const Node)).unwrap()};
                trace!("Active container set to view {:?}", view);
                active_container_set = true;
                break;
            }
        let maybe_parent = parent.get_ancestor_of_type(ContainerType::Container);
            if maybe_parent.is_none() {
                parent = parent.get_ancestor_of_type(ContainerType::Workspace).unwrap();
            } else {
                parent = maybe_parent.unwrap();
            }
        }
        // We didn't find another view, set to the default container
        if !active_container_set && parent.get_container_type() == ContainerType::Workspace {
            let container = &parent.get_children()[0];
            trace!("Active container set to container {:?}", container);
            unsafe { self.set_active_container(&*(container as *const Node)).unwrap() };
        }
    }
}

/// Attempts to lock the tree. If the Result is Err, then a thread that
/// previously had the lock panicked and potentially left the tree in a bad state
pub fn try_lock_tree() -> TreeResult {
    trace!("Locking the tree!");
    TREE.try_lock()
}
