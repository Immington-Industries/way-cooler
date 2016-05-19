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

lazy_static! {
    static ref TREE: Mutex<Tree> = {
        Mutex::new(Tree{
            root: Node::new(Container::new_root()),
            active_container: ptr::null_mut(),
        })
    };
}


pub struct Tree {
    root: Node,
    active_container: *mut Node,
}

unsafe impl Send for Tree {}

impl Tree {

    /// Moves the current active container to a new workspace
    pub fn move_container_to_workspace(&mut self, name: &str) {
        let container: Option<Node> = None;
        if let Some(sub_container) = self.get_active_container() {
            // NOTE Assumes workspace exists, fix this
            // NOTE Should not do this, because floating windows
            // Should do an if let
            let container = Some(sub_container.remove_from_parent()
                .expect("Could not remove container, was not part of tree"));
        }
        if let Some(container) = container {
            if let Some(workspace) = self.get_workspace_by_name(name) {
                // Assume workspace has a container
                let output = &mut workspace.get_children_mut()[0];
                output.new_child(container.get_val().clone());
            }
        }
    }

    /// Switch to the workspace with the give name
    pub fn switch_workspace(&mut self, name: &str ) {
        trace!("Switching to workspace {}", name);
        if let Some(old_workspace) = self.get_active_workspace() {
            old_workspace.set_visibility(false);
        }
        if self.active_container.is_null() {
            warn!("Not current active container, cannot switch workspace");
            return;
        }
        let current_workspace: *mut Node;
        {
            if let Some(_) = self.get_workspace_by_name(name) {
                trace!("Found workspace {}", name);
            } else {
                trace!("Adding workspace {}", name);
                self.add_workspace(name.to_string());
            }
            let new_current_workspace = self.get_workspace_by_name_mut(name).expect(ERR_BAD_TREE);
            new_current_workspace.set_visibility(true);
            // Set the first view to be focused, so the screen refreshes itself
            if new_current_workspace.get_children()[0].get_children().len() > 0 {
                trace!("Focusing view");
                match new_current_workspace.get_children_mut()[0].get_children_mut()[0]
                    .get_val().get_handle().expect(ERR_BAD_TREE) {
                    Handle::View(view) => view.focus(),
                    _ => {},
                }
            } else {
                WlcView::root().focus();
            }
            // Update the tree's pointer to the currently focused container
            current_workspace = &mut new_current_workspace.get_children_mut()[0] as *mut Node;
        }
        self.active_container = current_workspace;
    }

    /// Returns the currently viewed container.
    /// If multiple views are selected, the parent container they share is returned
    pub fn get_active_container(&self) -> Option<&mut Node> {
        if self.active_container.is_null() {
            None
        } else {
            unsafe {
                Some(&mut *self.active_container)
            }
        }
    }

    /// Get the monitor (output) that the active container is located on
    pub fn get_active_output(&self) -> Option<&mut Node> {
        if let Some(node) = self.get_active_container() {
            node.get_ancestor_of_type(ContainerType::Output)
        } else {
            None
        }
    }

    /// Get the workspace that the active container is located on
    pub fn get_active_workspace(&self) -> Option<&mut Node> {
        if let Some(container) = self.get_active_container() {
            if let Some(workspace) = container.get_ancestor_of_type(ContainerType::Workspace) {
                return Some(workspace)
            }
        }
        return None
    }

    /// Find the workspace node that has the given name
    pub fn get_workspace_by_name(&mut self, name: &str) -> Option<&mut Node> {
        for child in self.root.get_children_mut()[0].get_children_mut() {
            if child.get_val().get_name().expect(ERR_BAD_TREE) != name {
                continue
            }
            return Some(child);
        }
        return None
    }

    /// Find the workspace node that has the given name, with a mutable reference
    pub fn get_workspace_by_name_mut(&mut self, name: &str) -> Option<&mut Node> {
        for child in self.root.get_children_mut()[0].get_children_mut() {
            if child.get_val().get_name().expect(ERR_BAD_TREE) != name {
                continue
            }
            return Some(child);
        }
        return None
    }

    /// Make a new output container with the given WlcOutput.
    /// This is done when a new monitor is added
    pub fn add_output(&mut self, wlc_output: WlcOutput) {
        match self.root.new_child(Container::new_output(wlc_output)) {
            Ok(_) => {},
            Err(e) => error!("Could not add output: {:?}", e),
        }
    }

    /// Make a new workspace container with the given name.
    pub fn add_workspace(&mut self, name: String) {
        if let Some(output) = self.get_active_output() {
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
                    if let Err(e) = workspace.new_child(Container::new_container(geometry)) {
                        error!("Could not add container to workspace: {:?}",e );
                    }
                },
                Err(e) => error!("Could not add workspace: {:?}", e),
            }
        }
    }

    /// Make a new view container with the given WlcView, and adds it to
    /// the active workspace.
    pub fn add_view(&mut self, wlc_view: WlcView) {
        let mut maybe_new_view: *const Node = ptr::null();
        if let Some(current_workspace) = self.get_active_workspace() {
            trace!("Adding view {:?} to {:?}", wlc_view, current_workspace);
            if current_workspace.get_children().len() == 0 {
                let output = self.get_active_output()
                    .expect("Could not get active output");
                let output_size = output.get_val().get_geometry()
                    .expect("Could not get geometry from output").size;
                let mut geometry = wlc_view.get_geometry()
                    .expect("Could not get geometry from wlc view").clone();
                // Ensure that it starts within the bounds
                // Width
                if geometry.origin.x > output_size.w as i32 {
                    geometry.origin.x = output_size.w as i32 ;
                } else if geometry.origin.x < 0 {
                    geometry.origin.x = 0;
                }
                // Height
                if geometry.origin.y > output_size.h as i32 {
                    geometry.origin.y = output_size.h as i32;
                } else if geometry.origin.y < 0 {
                    geometry.origin.y = 0;
                }
                wlc_view.set_geometry(ResizeEdge::empty(), &geometry);
                match current_workspace.new_child(Container::new_container(geometry)) {
                    Ok(_) => {},
                    Err(e) => {
                        error!("Could not add workspace: {:?}", e);
                        return;
                    }
                }
            }
            let container = &mut current_workspace.get_children_mut()[0];
            match container.new_child(Container::new_view(wlc_view)) {
                Ok(view_node) => maybe_new_view = view_node as *const Node,
                Err(e) => {
                    error!("Could not add view to current workspace: {:?}", e);
                    return;
                }
            }
        };
        if ! maybe_new_view.is_null() {
            self.active_container = maybe_new_view as *mut Node;
        }
    }

    /// Remove the view container with the given view
    pub fn remove_view(&self, wlc_view: &WlcView) {
        if let Some(view) = self.root.find_view_by_handle(&wlc_view) {
            let parent = view.get_parent().expect(ERR_BAD_TREE);
            parent.remove_child(view);
        }
    }
}

pub fn try_lock_tree() -> TreeResult {
    trace!("Locking the tree!");
    TREE.try_lock()
}
