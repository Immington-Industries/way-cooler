//! Main module to handle the layout.
//! This is where the i3-specific code is.

use std::sync::{Mutex, MutexGuard, TryLockError};
use std::ptr;

use super::container::{Container, Handle, ContainerType};
use super::node::{Node};
use super::super::rustwlc::handle::{WlcView, WlcOutput};


pub type TreeErr = TryLockError<MutexGuard<'static, Tree>>;
pub type TreeResult = Result<MutexGuard<'static, Tree>, TreeErr>;

const ERR_BAD_TREE: &'static str = "Layout tree was in an invalid configuration";

lazy_static! {
    static ref TREE: Mutex<Tree> = {
        Mutex::new(Tree{
            root: Node::new(Container::new_root()),
            active_container: ptr::null(),
        })
    };
}


pub struct Tree {
    root: Node,
    active_container: *const Node,
}

unsafe impl Send for Tree {}

impl Tree {
    /// Switch to the workspace with the give name
    pub fn switch_workspace(&mut self, name: &str ) {
        trace!("Switching to workspace {}", name);
        if let Some(old_workspace) = self.get_active_workspace() {
            old_workspace.set_visibility(false);
        }
        let current_workspace: *const Node;
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
            current_workspace = &new_current_workspace.get_children()[0] as *const Node;
        }
        self.active_container = current_workspace;
    }

    /// Returns the currently viewed container.
    /// If multiple views are selected, the parent container they share is returned
    pub fn get_active_container(&self) -> Option<&Node> {
        if self.active_container.is_null() {
            None
        } else {
            unsafe {
                Some(&*self.active_container)
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
    pub fn get_workspace_by_name(&self, name: &str) -> Option<&Node> {
        for child in self.root.get_children()[0].get_children() {
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
        self.root.new_child(Container::new_output(wlc_output));
    }

    /// Make a new workspace container with the given name.
    pub fn add_workspace(&mut self, name: String) {
        let workspace = Container::new_workspace(name.to_string());
        let mut index = 0;
        if let Some(output) = self.get_active_output() {
            for (cur_index, child) in self.root.get_children().iter().enumerate() {
                if child == output {
                    index = cur_index;
                    break;
                }
            }
        }
        let workspace = self.root.get_children_mut()[index].new_child(workspace);
        workspace.new_child(Container::new_container());
    }

    /// Make a new view container with the given WlcView, and adds it to
    /// the active workspace.
    pub fn add_view(&mut self, wlc_view: WlcView) {
        let mut maybe_new_view: *const Node = ptr::null();
        if let Some(current_workspace) = self.get_active_workspace() {
            trace!("Adding view {:?} to {:?}", wlc_view, current_workspace);
            if current_workspace.get_children().len() == 0 {
                current_workspace.new_child(Container::new_container());
            }
            let container = &mut current_workspace.get_children_mut()[0];
            let view_node = container.new_child(Container::new_view(wlc_view));
            maybe_new_view = view_node as *const Node;
        };
        if ! maybe_new_view.is_null() {
            self.active_container = maybe_new_view as *const Node;
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
