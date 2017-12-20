//! Commands from the user to manipulate the tree

use std::fs::File;
use std::io::Read;

use super::{try_lock_tree, lock_tree, try_lock_action};
use super::{Action, ActionErr, Bar, Container, ContainerType,
            Direction, Handle, Layout, TreeError, ResizeErr, IncompleteBackground};
use super::core::borders::Borders;
use ::render::Renderable;
use super::Tree;
use ::registry;

use uuid::Uuid;
use rustwlc::{Point, Size, Geometry, ResizeEdge, WlcView, WlcOutput, ViewType,
              VIEW_BIT_UNMANAGED, VIEW_BIT_MODAL, VIEW_BIT_POPUP};
use rustwlc::input::pointer;
use rustc_serialize::json::{Json, ToJson};

pub type CommandResult = Result<(), TreeError>;

/* These commands are exported to take nothing and return nothing,
 * since they are the commands actually registered and usable over
 * the IPC/Lua thread.
 */

pub fn remove_active() {
    let mut handle_to_remove = None;
    if let Ok(mut tree) = try_lock_tree() {
        if let Some(container) = tree.0.get_active_container_mut() {
            match *container {
                Container::View { handle, .. } => {
                    handle_to_remove = Some(handle);
                    // Views shouldn't be removed from tree, that's handled by
                    // view_destroyed callback
                },
                _ => {}
            }
        }
        // views have it removed in view_destroyed callback
        // container should be removed here though.
        if handle_to_remove.is_none() {
            if let Err(err) = tree.0.remove_active() {
                warn!("Could not remove the active container! {:?}\n{:?}\n{:?}",
                      tree.0.get_active_container(), err, tree.0);
            };
        }
    }
    if let Some(handle) = handle_to_remove {
        handle.close();
    }
}

pub fn toggle_float() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.toggle_float().ok();
    }
}

pub fn toggle_float_focus() {
    if let Ok(mut tree) = try_lock_tree() {
        if let Err(err) = tree.toggle_floating_focus() {
            warn!("Could not float focus: {:#?}", err);
        }
    }
}

pub fn tile_switch() {
    if let Ok(mut tree) = try_lock_tree() {
        if let Some(id) = tree.active_id() {
            tree.toggle_cardinal_tiling(id).unwrap_or_else(|err| {
                warn!("Could not toggle cardinal tiling: {:#?}", err);
            });
        }
    }
}

pub fn split_vertical() {
    if let Ok(mut tree) = try_lock_tree() {
        debug!("Layout.SplitVertical()");
        tree.0.toggle_active_layout(Layout::Vertical).ok();
    }
}

pub fn split_horizontal() {
    debug!("Layout.SplitHorizontal()");
    if let Ok(mut tree) = try_lock_tree() {
        tree.0.toggle_active_layout(Layout::Horizontal).ok();
    }
}

pub fn tile_tabbed() {
    debug!("Layout.SplitTabbed()");
    if let Ok(mut tree) = try_lock_tree() {
        tree.0.set_active_layout(Layout::Tabbed).unwrap_or_else(|err| {
            warn!("Could not tile as tabbed: {:?}", err);
        })
    }
}

pub fn tile_stacked() {
    if let Ok(mut tree) = try_lock_tree() {
        debug!("Layout.SplitStacked()");
        tree.0.set_active_layout(Layout::Stacked).unwrap_or_else(|err| {
            warn!("Could not tile as stacked: {:?}", err);
        })
    }
}

pub fn fullscreen_toggle() {
    if let Ok(mut tree) = try_lock_tree() {
        if let Some(id) = tree.active_id() {
            let toggle = !tree.is_fullscreen(id)
                .expect("Active ID was invalid!");
            tree.set_fullscreen(id, toggle)
                .unwrap_or_else(|_| {
                    warn!("Could not set {:?} to fullscreen flag to be {:?}",
                          id, toggle);
                })
        }
    }
}

pub fn focus_left() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.move_focus(Direction::Left)
            .unwrap_or_else(|_| {
                warn!("Could not focus left");
            });
    }
}

pub fn focus_right() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.move_focus(Direction::Right)
            .unwrap_or_else(|_| {
                warn!("Could not focus right");
            });
    }
}

pub fn focus_up() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.move_focus(Direction::Up)
            .unwrap_or_else(|_| {
                warn!("Could not focus up");
            });
    }
}

pub fn focus_down() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.move_focus(Direction::Down)
            .unwrap_or_else(|_| {
                warn!("Could not focus down");
            });
    }
}

pub fn move_active_left() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.move_active(None, Direction::Left)
            .unwrap_or_else(|_| {
                warn!("Could not focus left");
            })
    }
}

pub fn move_active_right() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.move_active(None, Direction::Right)
            .unwrap_or_else(|_| {
                warn!("Could not focus right");
            })
    }
}

pub fn move_active_up() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.move_active(None, Direction::Up)
            .unwrap_or_else(|_| {
                warn!("Could not focus up");
            })
    }
}

pub fn move_active_down() {
    if let Ok(mut tree) = try_lock_tree() {
        tree.move_active(None, Direction::Down)
            .unwrap_or_else(|_| {
                warn!("Could not focus down");
            })
    }
}

pub fn tree_as_json() -> Json {
    if let Ok(tree) = lock_tree() {
        tree.0.to_json()
    } else {
        Json::Null
    }
}

/// Returns a copy of the action behind the lock.
pub fn performing_action() -> Option<Action> {
    if let Ok(action) = try_lock_action() {
        *action
    } else {
        warn!("Could not lock action mutex!");
        None
    }
}

/// Sets the value behind the lock to the provided value.
///
/// Note that this method blocks until the lock is released
///
/// None means an action is done being performed.
pub fn set_performing_action(val: Option<Action>) {
    if let Ok(mut action) = try_lock_action() {
        *action = val;
    } else {
        error!("Action mutex was poisoned");
        panic!("Action mutex was poisoned");
    }
}

// TODO Remove all instances of self.0.tree, that should be abstracted in LayoutTree.

/* These commands are the interface that the rest of Way Cooler has to the
 * tree. Any action done, whether through a callback, or from the IPC/Lua thread
 * it will have to go through one of these methods.
 */

/// These commands are the interface that the rest of Way Cooler has to the
/// tree. Any action done, whether through a callback, or from the IPC/Lua thread
/// it will have to go through one of these methods.
#[allow(dead_code)]
impl Tree {
    /// Gets the uuid of the active container, if there is an active container
    pub fn active_id(&self) -> Option<Uuid> {
        self.0.active_container
            .map(|active_ix| self.0.tree[active_ix].get_id())
    }

    pub fn lookup_handle(&self, handle: Handle) -> Result<Uuid, TreeError> {
        match handle {
            Handle::View(view) =>
                self.0.lookup_view(view).map(|c| c.get_id()),
            Handle::Output(_) => {
                let root_ix = self.0.tree.root_ix();
                let node_ix = self.0.tree.descendant_with_handle(root_ix, handle)
                    .ok_or(TreeError::HandleNotFound(handle))?;
                Ok(self.0.tree[node_ix].get_id())
            }
        }
    }

    /// Determines if the container is in the currently active workspace.
    pub fn container_in_active_workspace(&self, id: Uuid) -> Result<bool, TreeError> {
        let view = match try!(self.0.lookup(id)).get_handle()? {
            Handle::View(view) => view,
            _ => return Err(TreeError::UuidNotAssociatedWith(ContainerType::View))
        };
        if let Some(active_workspace) = self.0.active_ix_of(ContainerType::Workspace) {
            Ok(self.0.tree.descendant_with_handle(active_workspace, view.into()).is_some())
        } else {
            Ok(false)
        }
    }

    /// Gets a reference to the container that is the active workspace
    pub fn active_workspace(&self) -> Result<&Container, TreeError> {
        if let Some(node_ix) = self.0.active_ix_of(ContainerType::Workspace) {
            Ok(&self.0.tree[node_ix])
        } else {
            Err(TreeError::NoActiveContainer)
        }
    }

    pub fn toggle_float(&mut self) -> CommandResult {
        debug!("Layout.ToggleFloat()");
        if let Some(uuid) = self.active_id() {
            let is_floating: Result<bool, _> = self.0.lookup(uuid)
                .and_then(|container| Ok(container.floating()));
            try!(match is_floating {
                Ok(true) => self.ground_container(uuid),
                Ok(false) => self.float_container(uuid),
                Err(err) => return Err(err)
            });
        }
        Ok(())
    }

    /// Toggles between horizontal and vertical layout.
    ///
    /// If on neither, defaults to horizontal.
    pub fn toggle_cardinal_tiling(&mut self, id: Uuid) -> CommandResult {
        debug!("Layout.ToggleCardinalTiling()");
        self.0.toggle_cardinal_tiling(id)
            .and_then(|_| self.layout_active_of(ContainerType::Workspace))
    }

    /// Sets the active container to the given layout.
    pub fn set_active_layout(&mut self, layout: Layout) -> CommandResult {
        debug!("Layout.SetActiveLayout(\"{}\")", layout);
        self.0.set_active_layout(layout)
    }

    pub fn toggle_floating_focus(&mut self) -> CommandResult {
        debug!("Layout.ToggleFloatingFocus()");
        self.0.toggle_floating_focus()
    }

    pub fn move_active(&mut self, maybe_uuid: Option<Uuid>,
                       direction: Direction) -> CommandResult {
        let uuid = try!(maybe_uuid
                        .or_else(|| self.0.get_active_container()
                                 .and_then(|container| Some(container.get_id())))
                        .ok_or(TreeError::NoActiveContainer));
        debug!("Layout.MoveContainer(\"{}\", \"{}\")",
               uuid, direction);
        try!(self.0.move_container(uuid, direction));
        try!(self.layout_active_of(ContainerType::Output));
        Ok(())
    }

    /// Attempts to drag the window around the screen.
    pub fn try_drag_active(&mut self, point: Point) -> CommandResult {
        if let Some(mut action) = performing_action() {
            let old_point = action.grab;
            let active_ix = try!(self.0.active_container
                                .ok_or(TreeError::NoActiveContainer));
            try!(self.0.drag_floating(active_ix, point, old_point));
            action.grab = point;
            set_performing_action(Some(action));
            Ok(())
        } else {
            Err(TreeError::PerformingAction(false))
        }
    }

    /// Adds an Output to the tree. Never fails
    pub fn add_output(&mut self, output: WlcOutput) -> CommandResult {
        self.0.add_output(output)
    }

    /// Gets a list of UUIDs for all the outputs, in the order they were added.
    pub fn outputs(&self) -> Vec<Uuid> {
        let root_ix = self.0.tree.root_ix();
        self.0.tree.children_of(root_ix).iter()
            .map(|output_ix| self.0.tree[*output_ix].get_id())
            .collect()
    }

    pub fn output_resolution(&self, id: Uuid) -> Result<Size, TreeError> {
        let output = match try!(self.0.lookup(id)).get_handle()? {
            Handle::Output(output) => output,
            _ => return Err(TreeError::UuidNotAssociatedWith(
                ContainerType::Output))
        };
        Ok(output.get_resolution().expect("Output had no resolution"))
    }

    /// Binds a view to be the background for the given outputs.
    ///
    /// If there was a previous background, it is removed and deallocated.
    pub fn add_background(&mut self, view: WlcView, output: WlcOutput) -> Result<bool, TreeError> {
        let id;
        {
            let output_c = self.0.output_by_handle_mut(output)
                .ok_or(TreeError::OutputNotFound(output))?;
            id = output_c.get_id();
            match output_c.get_type() {
                ContainerType::Output => {},
                other => {
                    return Err(TreeError::UuidNotAssociatedWith(other))
                }
            };
        }
        self.0.attach_background(view, id)
    }


    /// Adds a background to an output.
    ///
    /// NOTE That the background is not ready yet, because to make it ready requires
    /// triggering the view callback. But, in order to not have it added twice we
    /// need to check if it's already been added...which isn't possible if the tree is locked.
    /// So, this will just add the meta data around it, and once it's added in the view callback
    /// it will add it "for real".
    /// Yes this is dumb, wlc can be kind of backwards...
    /// NOTE At the end of this method it calls the special wlc method that
    /// triggers the view_created callback. The view_callback needs to check that this
    pub fn add_incomplete_background(&mut self,
                                     background: IncompleteBackground,
                                     output: WlcOutput)
                                     -> CommandResult {
        let id;
        {
            let output_c = self.0.output_by_handle_mut(output)
                .ok_or(TreeError::OutputNotFound(output))?;
            id = output_c.get_id();
            match output_c.get_type() {
                ContainerType::Output => {},
                other => {
                    return Err(TreeError::UuidNotAssociatedWith(other))
                }
            };
        }
        self.0.attach_incomplete_background(background, id)
    }

    /// Adds a Workspace to the tree. Never fails
    pub fn switch_to_workspace(&mut self, name: &str) -> CommandResult {
        debug!("Layout.SwitchWorkspace(\"{}\")", name);
        self.0.switch_to_workspace(name);
        Ok(())
    }

    /// Gets the current workspace we are focused on
    pub fn current_workspace(&self) -> Result<&str, TreeError> {
        self.0.current_workspace()
    }

    /// Tiles the active container of some container type. Never fails
    pub fn layout_active_of(&mut self, c_type: ContainerType) -> CommandResult {
        self.0.layout_active_of(c_type);
        Ok(())
    }

    /// Attempts to set the node behind the id to be floating
    pub fn float_container(&mut self, id: Uuid) -> CommandResult {
        debug!("Layout.ToggleFloat(\"{}\")", id);
        self.0.float_container(id)
    }

    /// Attempts to set the node behind the id to be not floating
    pub fn ground_container(&mut self, id: Uuid) -> CommandResult {
        debug!("Layout.ToggleFloat(\"{}\")", id);
        self.0.ground_container(id)
    }

    /// Adds a view to the workspace of the active container
    pub fn add_view(&mut self, view: WlcView) -> CommandResult {
        let pid = view.get_pid();
        if let Ok(mut pid_f) = File::open(format!("/proc/{}/cmdline", pid)) {
            let mut program_name = String::new();
            pid_f.read_to_string(&mut program_name)
                .expect("Could not read file for process name");
            if program_name.ends_with('\0') {
                program_name.pop();
            }
            debug!("Layout.SpawnProgram(\"{}\")", program_name);
        }
        let tree = &mut self.0;
        let output = view.get_output();
        if tree.get_active_container().is_none() {
            return Err(TreeError::NoActiveContainer)
        }
        view.set_mask(output.get_mask());
        // If this view is a subsurface
        let has_parent = view.get_parent() != WlcView::root();
        let view_bit = view.get_type();
        trace!("Adding view: {:?}\n w/ bit: {:?}\n has parent: {:?}\n\
                title: {:?}\n class: {:?}\n appid: {:?}",
               view, view_bit.bits(), has_parent,
               view.get_title(), view.get_class(), view.get_app_id());
        if view_bit.intersects(VIEW_BIT_UNMANAGED) {
            tree.add_floating_view(view, None)?;
        } else if has_parent {
            match view_bit {
                VIEW_BIT_MODAL => {
                    let geo = view.get_geometry()
                        .expect("View had no geometry");
                    let borders = Borders::new(geo, output);
                    tree.add_floating_view(view, borders)?;
                    view.focus();
                },
                VIEW_BIT_POPUP => {
                    tree.add_floating_view(view, None)?;
                },
                v => {
                    if v == ViewType::empty() {
                        let geo = view.get_geometry()
                            .expect("View had no geometry");
                        let borders = Borders::new(geo, output);
                        tree.add_floating_view(view, borders)?;
                        view.focus();
                    } else {
                        tree.add_floating_view(view, None)?;
                    }
                }
            }
        } else if view_bit != ViewType::empty() {
            tree.add_floating_view(view, None)?;
        }
        else {
            tree.add_view(view)?;
            tree.normalize_view(view)?;
        }
        tree.layout_active_of(ContainerType::Workspace);
        Ok(())
    }

    /// Attempts to remove a view from the tree. If it is not in the tree it fails.
    ///
    /// # Safety
    /// This will **NOT** close the handle behind the view.
    /// The main use of this function is to be called from the `view_destroyed`
    /// callback. If you are calling this function from somewhere else,
    /// you should instead simply call `view.close`. This will be triggered
    /// in the callback.
    pub fn remove_view(&mut self, view: WlcView) -> CommandResult {
        let result;
        match self.0.remove_view(view) {
            Err(err)  => {
                result = Err(err)
            },
            Ok(Container::View { handle, id, .. }) => {
                trace!("Removed container {:?} with id {:?}",
                       handle, id);
                result = Ok(())
            },
            _ => unreachable!()
        }
        let root_ix = self.0.tree.root_ix();
        self.0.layout(root_ix);
        result
    }

    /// Attempts to remove a container based on UUID. Fails if the container
    /// cannot be removed or if the container does not exist.
    ///
    /// This WILL close the view, and should never be called from the
    /// `view_destroyed` callback, as it's possible the view from that callback is invalid.
    pub fn remove_view_by_id(&mut self, id: Uuid) -> CommandResult {
        debug!("Layout.CloseView(\"{}\")", id);
        match self.0.lookup(id)?.get_handle()? {
            Handle::View(view) => {
                view.close();
                Ok(())
            },
            Handle::Output(_) =>
                Err(TreeError::UuidNotAssociatedWith(ContainerType::View))
        }
    }

    pub fn update_title(&mut self, view: WlcView) -> CommandResult {
        let id = try!(self.lookup_handle(view.into())
                      .map_err(|_|TreeError::ViewNotFound(view)));
        {
            let container = try!(self.0.lookup_mut(id));
            container.set_name(Container::get_title(view));
            container.draw_borders()?;
        }
        // Update the parent container using draw_borders_rec
        let node_ix = self.0.tree.lookup_id(id)
            .ok_or(TreeError::ViewNotFound(view))?;
        self.0.draw_borders_rec(vec![node_ix])?;
        Ok(())
    }

    /// Sets the view to be the new active container.
    /// Will fail if the container is floating.
    pub fn set_active_view(&mut self, view: WlcView) -> CommandResult {
        try!(self.0.set_active_view(view.clone()));
        view.focus();
        Ok(())
    }

    /// Resets the focus to be whatever the active path points to.
    /// This is useful when the `active_container` is `None`, e.g when
    /// closing the lock screen.
    pub fn reset_focus(&mut self) -> CommandResult {
        let root_ix = self.0.tree.root_ix();
        let to_focus = self.0.tree.follow_path(root_ix);
        let to_focus_id = self.0.tree[to_focus].get_id();
        self.0.focus_on(to_focus_id)?;
        Ok(())
    }

    #[allow(dead_code)]
    /// Sets the active container to be the container at the UUID
    /// Fails if the container is not a container or view, or if the
    /// container does not exist
    ///
    /// Can also not set floating containers to be active.
    pub fn set_active_container_by_id(&mut self, id: Uuid) -> CommandResult {
        debug!("Layout.Focus(\"{}\")", id);
        self.0.set_active_container(id)
    }

    /// Sets the container behind the UUID to be fullscreen.
    ///
    /// If the container is a non-View/Container, then an error is returned
    /// and the flag is not set (it's only tracked for Views and Containers).
    pub fn set_fullscreen(&mut self, id: Uuid, toggle: bool) -> CommandResult {
        debug!("Layout.FullScreen(\"{}\", {})", id, toggle);
        {
            let container = try!(self.0.lookup_mut(id));
            try!(container.set_fullscreen(toggle)
                 .map_err(|_| TreeError::UuidWrongType(id, vec![ContainerType::View,
                                                                ContainerType::Container])));
        }
        // Now update the workspace so that it knows which children are fullscreen
        {
            /*
            TODO This needs proper error handling!
            Correct way is to have active_ix_of to return a proper `Result<NodeIndex, TreeError>`
            however that would break too much code and needs a branch to itself.

            For now, in order to have the IPC not break Way Cooler,
            this will return an incorrect error.
            */
            if let Some(workspace_ix) = self.0.active_ix_of(ContainerType::Workspace) {
                let workspace = &mut self.0.tree[workspace_ix];
                workspace.update_fullscreen_c(id, toggle).map_err(
                    |_| TreeError::UuidWrongType(workspace.get_id(),
                                                 vec![ContainerType::Workspace]))?;
            } else {
                // WRONG ID! See TODO above
                return Err(TreeError::UuidWrongType(id, vec![ContainerType::Workspace]))
            }
        }
        // TODO Only do this if in path, or otherwise just tile the workspace the container is in
        // MOST of the time, this isn't a useless operation (read: when keybinding is used),
        // but still the IPC shouldn't do a useless tile if it can help it.
        self.layout_active_of(ContainerType::Workspace)
    }

    pub fn is_fullscreen(&self, id: Uuid) -> Result<bool, TreeError> {
        let container = try!(self.0.lookup(id));
        Ok(container.fullscreen())
    }

    /// Focuses on the container. If the container is not floating and is a
    /// Container or a View, then it is also made the active container.
    pub fn focus(&mut self, id: Uuid) -> CommandResult {
        debug!("Layout.focus(\"{}\")", id);
        self.set_active_container_by_id(id)
            .or_else(|_| {
                self.0.focus_on(id)
            })
    }

    /// Destroy the tree
    pub fn destroy_tree(&mut self) -> CommandResult {
        self.0.destroy_tree();
        Ok(())
    }

    pub fn move_focus(&mut self, dir: Direction) -> CommandResult {
        debug!("Layout.FocusDir(\"{}\")", dir);
        self.0.move_focus(dir)?;
        // NOTE Since tiling is somewhat expensive,
        // this can be a bottleneck that can be possibly optimized.
        self.0.layout_active_of(ContainerType::Workspace);
        Ok(())
    }

    /// Moves the active container to a workspace
    pub fn send_active_to_workspace(&mut self, workspace_name: &str) -> CommandResult {
        debug!("Layout.SendActiveToWorkspace(\"{}\")", workspace_name);
        self.0.send_active_to_workspace(workspace_name);
        Ok(())
    }

    /// Resizes the container, as if it was dragged at the edge to a certain point
    /// on the screen.
    pub fn resize_container(&mut self, id: Uuid, edge: ResizeEdge, pointer: Point)
                            -> CommandResult {
        match try_lock_action() {
            Ok(mut lock) => {
                if let Some(ref mut action) = *lock {
                    if try!(self.0.lookup(id)).floating() {
                        self.0.resize_floating(id, edge, pointer, action)
                    } else {
                        let new_point = self.0.resize_tiled(id, edge, pointer, action)?;
                        // look up mouse lock option
                        let lock = registry::clients_read();
                        let client = lock.client(Uuid::nil()).unwrap();
                        let handle = registry::ReadHandle::new(&client);
                        let lock_mouse = handle.read("mouse".into())
                            .expect("mouse category didn't exist")
                            .get("lock_to_corner_on_resize".into())
                            .and_then(|data| data.as_boolean()).unwrap_or(false);
                        if lock_mouse {
                            action.grab = new_point
                        } else {
                            pointer::set_position_v2(pointer.x as f64, pointer.y as f64);
                        }
                        Ok(())
                    }
                } else {
                    Err(TreeError::Action(ActionErr::ActionNotInProgress))
                }
            },
            _ => Err(TreeError::Action(ActionErr::ActionLocked))
        }
    }

    pub fn send_to_workspace(&mut self, id: Uuid, workspace_name: &str) -> CommandResult {
        if self.0.tree.lookup_id(id).is_none() {
            Err(::layout::GraphError::LookupFailed(id))?
        }
        debug!("Layout.SendToWorkspace(\"{}\", \"{}\")", id, workspace_name);
        self.0.send_to_workspace(id, workspace_name);
        Ok(())
    }

    pub fn set_pointer_pos(&mut self, point: Point) -> CommandResult {
        self.0.set_pointer_pos(point)
    }

    pub fn grab_at_corner(&mut self, id: Uuid, edge: ResizeEdge) -> CommandResult {
        self.0.grab_at_corner(id, edge)
            .and(Ok(()))
    }

    /// Adds the view as a bar to the specified output
    ///
    /// For more information, see bar.rs and container.rs
    pub fn add_bar(&mut self, view: WlcView, output: WlcOutput) -> CommandResult {
        let result = if let Some(output_c) = self.0.output_by_handle_mut(output) {
            match *output_c {
                Container::Output { ref mut bar, .. } => {
                    let new_bar = Bar::new(view);
                    *bar = Some(new_bar);
                },
                _ => unreachable!()
            }
            Ok(())
        } else {
            Err(TreeError::OutputNotFound(output))
        };
        result.and_then(|_| {
            self.0.layout_active_of(ContainerType::Output);
            Ok(())
        })
   }

    /// Updates the geometry of the view from an external request
    /// (such a request can come from the view itself)
    pub fn update_floating_geometry(&mut self, view: WlcView,
                                    mut geometry: Geometry) -> CommandResult {
        let container = self.0.lookup_view_mut(view)?;
        if container.floating() {
            // If we didn't request it to be at 0,0, don't move
            // FIXME WORKAROUND This is a workaround where certain popups
            // (I'm looking at you, Firefox save), will request it to be at 0,0
            // but with the correct size. This causes a race condition,
            // where sometimes we get to update the origin first and sometimes
            // the client does.

            // NOTE This gets the "effective_geometry",
            // so it's only updated from Way Cooler's side.
            let effective_geo = container.get_geometry()
                .expect("Updated a container that wasn't a view!");
            if effective_geo.origin != geometry.origin {
                // And it's trying to put it in the top left.
                if geometry.origin == Point::new(0, 0) {
                    let output = view.get_output();
                    let res = output.get_resolution()
                        .expect("Output had no resolution");
                    geometry.origin.x = (res.w / 2 - geometry.size.w / 2) as i32;
                    geometry.origin.y = (res.h / 2 - geometry.size.h / 2) as i32;
                }
            }
            container.set_geometry(ResizeEdge::empty(), geometry);
            container.resize_borders(geometry);
            container.draw_borders()?;
            Ok(())
        } else {
            let uuid = container.get_id();
            Err(ResizeErr::ExpectedFloating(uuid))?
        }
    }

    /// Renders the borders for the view.
    pub fn render_borders(&mut self, view: WlcView) -> CommandResult {
        let node_ix = try!(self.lookup_handle(view.into()).ok()
                      .and_then(|id| self.0.tree.lookup_id(id))
                      .ok_or_else(||TreeError::ViewNotFound(view)));
        let floating = {
            let container = &mut self.0.tree[node_ix];
            container.render_borders();
            container.floating()
        };
        self.0.tree[node_ix].render_borders();
        // Render parent container too, if applicable.
        let mut parent_ix = self.0.tree.parent_of(node_ix)?;
        if ! floating {
            while self.0.tree[parent_ix].get_type() != ContainerType::Workspace {
                self.0.tree[parent_ix].render_borders();
                parent_ix = self.0.tree.parent_of(parent_ix)?
            }
        }
        Ok(())
    }
}
