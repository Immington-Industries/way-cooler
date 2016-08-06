//! Commands from the user to manipulate the tree

use super::try_lock_tree;
use super::{ContainerType, Direction, Handle, Layout};
use super::Tree;

use uuid::Uuid;
use rustwlc::{Geometry, Point, ResizeEdge, WlcView, WlcOutput, ViewType};

pub type CommandResult = Result<(), String>;

pub fn remove_active() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.0.remove_active();
    }
}

pub fn tile_switch() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.0.toggle_active_horizontal();
        tree.layout_active_of(ContainerType::Workspace)
            .unwrap_or_else(|_| {
                error!("Could not tile workspace");
            });
    }
}

pub fn split_vertical() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.0.toggle_active_layout(Layout::Vertical);
    }
}

pub fn split_horizontal() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.0.toggle_active_layout(Layout::Horizontal);
    }
}

pub fn focus_left() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.move_focus(Direction::Left)
            .unwrap_or_else(|_| {
                error!("Could not focus left");
            });
    }
}

pub fn focus_right() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.move_focus(Direction::Right)
            .unwrap_or_else(|_| {
                error!("Could not focus right");
            });
    }
}

pub fn focus_up() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.move_focus(Direction::Up)
            .unwrap_or_else(|_| {
                error!("Could not focus up");
            });
    }
}

pub fn focus_down() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.move_focus(Direction::Down)
            .unwrap_or_else(|_| {
                error!("Could not focus down");
            });
    }
}

/* Commands that can be chained together with a locked tree */

impl Tree {

    /// Adds an Output to the tree. Never fails
    pub fn add_output(&mut self, output: WlcOutput) -> CommandResult {
        self.0.add_output(output);
        Ok(())
    }

    /// Adds a Workspace to the tree. Never fails
    pub fn switch_to_workspace(&mut self, name: &str) -> CommandResult {
        self.0.switch_to_workspace(name);
        Ok(())
    }

    /// Tiles the active container of some container type. Never fails
    pub fn layout_active_of(&mut self, c_type: ContainerType) -> CommandResult {
        self.0.layout_active_of(c_type);
        Ok(())
    }

    /// Adds a view to the workspace of the active container
    pub fn add_view(&mut self, view: WlcView) -> CommandResult {
        let tree = &mut self.0;
        let output = view.get_output();
        if tree.get_active_container().is_none() {
            return Err(format!("No active container, cannot add view {:?} to output {:?}!", view, output))
        }
        view.set_mask(output.get_mask());
        let v_type = view.get_type();
        let v_class = view.get_class();
        // If it is empty, don't add to tree
        if v_type != ViewType::empty() {
            // Now focused on something outside the tree,
            // have to unset the active container
            if !tree.active_is_root() {
                tree.unset_active_container();
            }
            return Ok(())
        }
        if v_class.as_str() == "Background" {
            info!("Setting background: {}", view.get_title());
            view.send_to_back();
            let output = view.get_output();
            let resolution = output.get_resolution().clone();
            let fullscreen = Geometry {
                origin: Point { x: 0, y: 0 },
                size: resolution
            };
            view.set_geometry(ResizeEdge::empty(), &fullscreen);
            return Ok(());
        }
        tree.add_view(view.clone());
        tree.normalize_view(view.clone());
        tree.layout_active_of(ContainerType::Container);
        Ok(())
    }

    /// Attempts to remove a view from the tree. If it is not in the tree it fails
    pub fn remove_view(&mut self, view: WlcView) -> CommandResult {
        self.0.remove_view(&view)
    }

    #[allow(dead_code)]
    /// Attempts to remove a container based on UUID. Fails if the container
    /// cannot be removed or if the container does not exist.
    pub fn remove_view_by_id(&mut self, id: Uuid) -> CommandResult {
        if let Some(node_ix) = self.0.tree.lookup_id(id) {
            match self.0.tree[node_ix].get_type() {
                ContainerType::View => {
                    let handle = match self.0.tree[node_ix].get_handle()
                        .expect("Could not get handle") {
                            Handle::View(ref handle) => handle.clone(),
                            _ => unreachable!()
                        };
                    return self.remove_view(handle)
                },
                _ => {
                    Err("That UUID was not associated with a view".into())
                }
            }
        } else {
            Err("Could not find container".into())
        }
    }

    /// Sets the view to be the new active container. Never fails
    pub fn set_active_view(&mut self, view: WlcView) -> CommandResult {
        self.0.set_active_container(view);
        Ok(())
    }

    #[allow(dead_code)]
    /// Sets the active container to be the container at the UUID
    /// Fails if the container is not a container or view, or if the
    /// container does not exist
    pub fn set_active_container_by_id(&mut self, id: Uuid) -> CommandResult {
        if let Some(node_ix) = self.0.tree.lookup_id(id) {
            match self.0.tree[node_ix].get_type() {
                ContainerType::View | ContainerType::Container => {
                    self.0.active_container = Some(node_ix);
                    Ok(())
                },
                _ => {
                    Err(format!("UUID did not point to view, it pointed to index {:?}, which points to {:?}",
                                node_ix, self.0.tree[node_ix]))
                }

            }
        } else {
            Err("Could not find container".into())
        }
    }

    /// Destroy the tree
    pub fn destroy_tree(&mut self) -> CommandResult {
        self.0.destroy_tree();
        Ok(())
    }

    pub fn move_focus(&mut self, dir: Direction) -> CommandResult {
        self.0.move_focus(dir);
        Ok(())
    }

    /// Moves the active container to a workspace
    pub fn send_active_to_workspace(&mut self, workspace_name: &str) -> CommandResult {
        self.0.send_active_to_workspace(workspace_name);
        Ok(())
    }
}
