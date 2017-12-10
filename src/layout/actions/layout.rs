use std::cmp;
use std::collections::HashSet;
use std::iter::FromIterator;

use petgraph::graph::NodeIndex;
use rustwlc::{WlcView, Geometry, Point, Size, ResizeEdge};

use super::super::{LayoutTree, TreeError};
use super::super::commands::CommandResult;
use super::super::core::container::{Container, ContainerType, ContainerErr,
                                    Layout, Handle};
use super::super::core::background::MaybeBackground;
use super::borders;
use ::layout::core::borders::Borders;
use ::render::Renderable;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LayoutErr {
    /// The node behind the UUID was asked to ground when it was already grounded.
    AlreadyGrounded(Uuid),
    /// The node behind the UUID was asked to float when it was already floating.
    AlreadyFloating(Uuid)
}

impl LayoutTree {
    /// Given the index of some container in the tree, lays out the children of
    /// that container based on what type of container it is and how big of an
    /// area is allocated for it and its children.
    pub fn layout(&mut self, node_ix: NodeIndex) {
        match self.tree[node_ix].get_type() {
            ContainerType::Root => {
                for output_ix in self.tree.children_of(node_ix) {
                    self.layout(output_ix);
                }
            }
            ContainerType::Output => {
                let geometry;
                {
                    let container = &mut self.tree[node_ix];
                    geometry = container.get_geometry()
                        .expect("Output had no geometry");
                    let actual_geometry = container.get_actual_geometry()
                        .expect("Output had no actual geometry");
                    match *container {
                        Container::Output { ref mut background, .. } => {
                            // update the background size
                            match *background {
                                Some(MaybeBackground::Complete(background)) => {
                                    background.handle.set_geometry(ResizeEdge::empty(), actual_geometry)
                                },
                                _ => {}
                            }
                        }
                        _ => unreachable!()
                    }
                }
                let mut fullscreen_apps = Vec::new();
                for workspace_ix in self.tree.children_of(node_ix) {
                    self.layout_helper(workspace_ix, geometry, &mut fullscreen_apps);
                }
                self.layout_fullscreen_apps(fullscreen_apps);
            }
            ContainerType::Workspace => {
                // get geometry from the parent output
                let output_ix = self.tree.ancestor_of_type(node_ix, ContainerType::Output)
                    .expect("Workspace had no output parent");
                let output_geometry = self.tree[output_ix].get_geometry()
                    .expect("Could not get output geometry");
                let mut fullscreen_apps = Vec::new();
                self.layout_helper(node_ix, output_geometry, &mut fullscreen_apps);
                self.layout_fullscreen_apps(fullscreen_apps)
            }
            ContainerType::Container => {
                let geometry = self.tree[node_ix].get_actual_geometry()
                    .expect("Could not get actual container geometry");
                // TODO Fake vector that doesn't allocate for this case?
                let mut fullscreen_apps = Vec::new();
                self.layout_helper(node_ix, geometry, &mut fullscreen_apps);
            }
            ContainerType::View => {
                let parent_ix = self.tree.parent_of(node_ix)
                    .expect("View had no parent");
                self.layout(parent_ix);
            }
        }
        self.validate();
    }

    /// Helper function to layout a container. The geometry is the constraint geometry,
    /// the container tries to lay itself out within the confines defined by the constraint.
    /// Generally, this should not be used directly and layout should be used.
    fn layout_helper(&mut self, node_ix: NodeIndex, mut geometry: Geometry,
                     fullscreen_apps: &mut Vec<NodeIndex>) {
        if self.tree[node_ix].fullscreen() {
            fullscreen_apps.push(node_ix);
        }
        match self.tree[node_ix].get_type() {
            ContainerType::Root => {
                warn!("Ignoring geometry constraint ({:#?}), \
                       deferring to each output's constraints",
                      geometry);
                for child_ix in self.tree.children_of(node_ix) {
                    self.layout(child_ix);
                }
            },
            ContainerType::Output => {
                self.tree[node_ix].set_geometry(ResizeEdge::empty(), geometry);
                for child_ix in self.tree.children_of(node_ix) {
                    self.layout_helper(child_ix, geometry, fullscreen_apps);
                }
            }
            ContainerType::Workspace => {
                self.tree[node_ix].set_geometry(ResizeEdge::empty(), geometry);
                for child_ix in self.tree.grounded_children(node_ix) {
                    self.layout_helper(child_ix, geometry, fullscreen_apps);
                }
                // place floating children above everything else
                let root_ix = self.tree.children_of(node_ix)[0];
                for child_ix in self.tree.floating_children(root_ix) {
                    // TODO Propogate error
                    self.place_floating(child_ix, fullscreen_apps).ok();
                }
            },
            ContainerType::Container => {
                // Update the geometry so that borders are included when tiling.
                geometry = self.update_container_geo_for_borders(node_ix, geometry)
                    .expect("Could not update container geo for tiling");

                let layout = match self.tree[node_ix] {
                    Container::Container { layout, .. } => layout,
                    _ => unreachable!()
                };
                match layout {
                    Layout::Horizontal => {
                        let children = self.tree.grounded_children(node_ix);
                        let children_len = children.len();
                        let mut scale = LayoutTree::calculate_scale(children.iter().map(|child_ix| {
                            let c_geometry = self.tree[*child_ix].get_geometry()
                                .expect("Child had no geometry");
                            c_geometry.size.w as f32
                        }).collect(), geometry.size.w as f32);

                        if scale > 0.1 {
                            scale = geometry.size.w as f32 / scale;
                            let new_size_f = |child_size: Size, sub_geometry: Geometry| {
                                let width = if child_size.w > 0 {
                                    child_size.w as f32
                                } else {
                                    // If the width would become zero, just make it the average size of the container.
                                    // e.g, if container was width 500 w/ 2 children, this view would have a width of 250
                                    geometry.size.w as f32 / children_len.checked_sub(1).unwrap_or(1) as f32
                                };
                                Size {
                                    w: ((width) * scale) as u32,
                                    h: sub_geometry.size.h
                                }
                            };
                            let remaining_size_f = |sub_geometry: Geometry,
                                                    cur_geometry: Geometry| {
                                let remaining_width =
                                    cur_geometry.origin.x + cur_geometry.size.w as i32 -
                                    sub_geometry.origin.x;
                                Size {
                                    w: remaining_width as u32,
                                    h: sub_geometry.size.h
                                }
                            };
                            let new_point_f = |new_size: Size, sub_geometry: Geometry| {
                                Point {
                                    x: sub_geometry.origin.x + new_size.w as i32,
                                    y: sub_geometry.origin.y
                                }
                            };
                            self.generic_tile(node_ix, geometry, children.as_slice(),
                                              new_size_f, remaining_size_f, new_point_f,
                                              fullscreen_apps);
                            self.add_gaps(node_ix)
                                .expect("Couldn't add gaps to horizontal container");
                            // TODO Propogate error
                            self.draw_borders_rec(children).ok();
                        }
                    }
                    Layout::Vertical => {
                        let children = self.tree.grounded_children(node_ix);
                        let children_len = children.len();
                        let mut scale = LayoutTree::calculate_scale(children.iter().map(|child_ix| {
                            let c_geometry = self.tree[*child_ix].get_geometry()
                                .expect("Child had no geometry");
                            c_geometry.size.h as f32
                        }).collect(), geometry.size.h as f32);

                        if scale > 0.1 {
                            scale = geometry.size.h as f32 / scale;
                            let new_size_f = |child_size: Size, sub_geometry: Geometry| {
                                let height = if child_size.h > 0 {
                                    child_size.h as f32
                                } else {
                                    // If the height would become zero, just make it the average size of the container.
                                    // e.g, if container was height 500 w/ 2 children, this view would have a height of 250
                                    geometry.size.h as f32 / children_len.checked_sub(1).unwrap_or(1) as f32
                                 };
                                Size {
                                    w: sub_geometry.size.w,
                                    h: ((height) * scale) as u32
                                }
                            };
                            let remaining_size_f = |sub_geometry: Geometry,
                                                    cur_geometry: Geometry| {
                                let remaining_height =
                                    cur_geometry.origin.y + cur_geometry.size.h as i32 -
                                    sub_geometry.origin.y;
                                Size {
                                    w: sub_geometry.size.w,
                                    h: remaining_height as u32
                                }
                            };
                            let new_point_f = |new_size: Size, sub_geometry: Geometry| {
                                Point {
                                    x: sub_geometry.origin.x,
                                    y: sub_geometry.origin.y + new_size.h as i32
                                }
                            };
                            self.generic_tile(node_ix, geometry, children.as_slice(),
                                              new_size_f, remaining_size_f, new_point_f,
                                              fullscreen_apps);
                            self.add_gaps(node_ix)
                                .expect("Couldn't add gaps to vertical container");
                            // TODO Propogate error
                            self.draw_borders_rec(children).ok();
                        }
                    },
                    Layout::Tabbed | Layout::Stacked => {
                        let workspace_ix = self.tree.ancestor_of_type(
                            node_ix, ContainerType::Workspace)
                            .expect("Node did not have a workspace as an ancestor");
                        // If we are on the wrong workspace, don't do any tiling.
                        if !self.tree.on_path(workspace_ix) {
                            return
                        }
                        // Set everything invisible,
                        // set floating and focused view to be visible.
                        let mut children = self.tree
                            .children_of_by_active(node_ix);
                        let mut seen = false;
                        // Pre-optimization, mostly < 7 floating views.
                        let mut views_to_vis = Vec::with_capacity(8);
                        for child_ix in &children {
                            if self.tree[*child_ix].floating() {
                                views_to_vis.push(*child_ix);
                                continue
                            }
                            if !seen {
                                seen = true;
                                views_to_vis.push(*child_ix);
                            }
                            self.layout_helper(*child_ix,
                                               geometry,
                                               fullscreen_apps);
                        }
                        self.set_container_visibility(node_ix, false);
                        for child_ix in views_to_vis {
                            self.set_container_visibility(child_ix, true);
                        }
                        children.push(node_ix);
                        // TODO Propogate error
                        self.add_gaps(node_ix)
                            .expect("Couldn't add gaps to tabbed/stacked container");
                        self.draw_borders_rec(children).ok();
                    },
                }
            }

            ContainerType::View => {
                self.tree[node_ix].set_geometry(ResizeEdge::empty(), geometry);
                self.update_view_geo_for_borders(node_ix)
                    .expect("Couldn't add border gaps to horizontal container");
            }
        }
        self.validate();
    }

    /// Attempts to set the node behind the id to be floating.
    ///
    /// This removes the container from its parent and makes its new parent-
    /// the workspace it resides in.
    ///
    /// The view will have a geometry of 1/2 the height/width, and set right in the
    /// middle of the screen.
    ///
    /// This will change the active container, but **not** the active path,
    /// it will remain pointing at the previous parent container.
    pub fn float_container(&mut self, id: Uuid) -> CommandResult {
        let node_ix = try!(self.tree.lookup_id(id).ok_or(TreeError::NodeNotFound(id)));
        if self.tree.is_root_container(node_ix) {
            return Err(TreeError::InvalidOperationOnRootContainer(id))
        }
        if self.tree[node_ix].floating() {
            warn!("Trying to float an already floating container");
            return Err(TreeError::Layout(LayoutErr::AlreadyFloating(id)));
        }
        let output_ix = try!(self.tree.ancestor_of_type(node_ix, ContainerType::Output)
                             .map_err(|err| TreeError::PetGraph(err)));
        let output_size = match self.tree[output_ix] {
            Container::Output { handle, .. } => {
                handle.get_resolution().expect("Output had no resolution")
            },
            _ => unreachable!()
        };
        {
            let container = &mut self.tree[node_ix];
            try!(container.set_floating(true)
                .map_err(|_| TreeError::UuidWrongType(id, vec!(ContainerType::View,
                                                                ContainerType::Container))));
            let new_geometry = Geometry {
                    size: Size {
                        h: output_size.h / 2,
                        w: output_size.w / 2
                    },
                    origin: Point {
                        x: (output_size.w / 2 - output_size.w / 4) as i32 ,
                        y: (output_size.h / 2 - output_size.h / 4) as i32
                    }
                };
            match container.get_type() {
                ContainerType::View | ContainerType::Container => {
                    container.set_geometry(ResizeEdge::empty(), new_geometry);
                },
                _ => return Err(TreeError::UuidWrongType(id, vec!(ContainerType::View,
                                                                ContainerType::Container)))
            }
            container.resize_borders(new_geometry);
            container.draw_borders()?;
        }
        let root_ix = self.tree.root_ix();
        let root_c_ix = try!(self.tree.follow_path_until(root_ix, ContainerType::Container)
                             .map_err(|_| TreeError::NoActiveContainer));
        let parent_ix = self.tree.parent_of(node_ix)
            .expect("View had no parent node!");
        try!(self.tree.move_into(node_ix, root_c_ix)
             .map_err(|err| TreeError::PetGraph(err)));
        self.tree.set_ancestor_paths_active(node_ix);
        if self.tree.can_remove_empty_parent(parent_ix) {
            try!(self.remove_view_or_container(parent_ix));
        }
        let parent_ix = self.tree.parent_of(root_c_ix).unwrap();
        self.layout(parent_ix);
        Ok(())
    }

    pub fn ground_container(&mut self, id: Uuid) -> CommandResult {
        let floating_ix = try!(self.tree.lookup_id(id).ok_or(TreeError::NodeNotFound(id)));
        if !self.tree[floating_ix].floating() {
            warn!("Trying to ground an already grounded container");
            return Err(TreeError::Layout(LayoutErr::AlreadyGrounded(id)));
        }
        let root_ix = self.tree.root_ix();
        let mut node_ix = self.tree.follow_path(root_ix);
        // If view, need to make it a sibling
        if self.tree[node_ix].get_type() == ContainerType::View {
            node_ix = try!(self.tree.parent_of(node_ix)
                           .map_err(|err| TreeError::PetGraph(err)));
        }
        {
            let container = &mut self.tree[floating_ix];
            try!(container.set_floating(false)
                 .map_err(|_| TreeError::UuidWrongType(id, vec!(ContainerType::View,
                                                                ContainerType::Container))));
        }
        try!(self.tree.move_into(floating_ix, node_ix)
             .map_err(|err| TreeError::PetGraph(err)));
        self.normalize_container(node_ix).ok();
        let root_ix = self.tree.root_ix();
        let root_c_ix = try!(self.tree.follow_path_until(root_ix, ContainerType::Container)
                             .map_err(|_| TreeError::NoActiveContainer));
        let parent_ix = self.tree.parent_of(root_c_ix).unwrap();
        self.layout(parent_ix);
        Ok(())
    }

    /// If the node is floating, places it at its reported position, above all
    /// other nodes.
    fn place_floating(&mut self, node_ix: NodeIndex,
                      fullscreen_apps: &mut Vec<NodeIndex>) -> CommandResult {
        if self.tree[node_ix].fullscreen() {
            fullscreen_apps.push(node_ix);
            return Ok(())
        }
        if !self.tree[node_ix].floating() {
            Err(ContainerErr::BadOperationOn(
                self.tree[node_ix].get_type(),
                "Tried to absolutely place a non-floating view!".into()))?
        }
        {
            let container = &mut self.tree[node_ix];
            match *container {
                Container::Container { .. } => { unimplemented!() },
                Container::View { ref handle, .. } => {
                    handle.bring_to_front();
                },
                _ => unreachable!()
            }
            container.draw_borders()?;
        }
        for child_ix in self.tree.floating_children(node_ix) {
            self.place_floating(child_ix, fullscreen_apps)?;
        }
        Ok(())
    }

    /// Changes the layout of the active container to the given layout.
    /// If the active container is a view, a new container is added with the given
    /// layout type.
    pub fn toggle_active_layout(&mut self, new_layout: Layout) -> CommandResult {
        if let Some(active_ix) = self.active_container {
            let parent_ix = self.tree.parent_of(active_ix)
                .expect("Active container had no parent");
            if self.tree.is_root_container(active_ix) {
                self.set_layout(active_ix, new_layout);
                return Ok(())
            }
            if self.tree.grounded_children(parent_ix).len() == 1 {
                // NOTE Do _NOT_ use set_layout,
                // the normalization causes the issue described in #344
                match self.tree[parent_ix] {
                    Container::Container { ref mut layout , ref mut borders, ..} => {
                        *layout = new_layout;
                        // This needs to be done manually because
                        // apparently set_layout cannot be used.
                        if let Some(ref mut _b) = borders.as_mut() {
                            _b.layout = Some(new_layout);
                        }
                    },
                    _ => unreachable!()
                }
                return Ok(())
            }

            let active_geometry = self.get_active_container()
                .expect("Could not get the active container")
                .get_geometry().expect("Active container had no geometry");
            let output_ix = self.tree.ancestor_of_type(active_ix,
                                                    ContainerType::Output)?;
            let output = match self.tree[output_ix].get_handle()? {
                Handle::Output(handle) => handle,
                _ => unreachable!()
            };
            let borders = Borders::new(active_geometry, output);
            let output_c = self.tree.ancestor_of_type(active_ix,
                                                      ContainerType::Output)?;
            let output_handle = match self.tree[output_c].get_handle()? {
                Handle::Output(output) => output,
                _ => unreachable!()
            };
            let mut new_container = Container::new_container(active_geometry,
                                                             output_handle,
                                                             borders);
            new_container.set_layout(new_layout).ok();
            self.add_container(new_container, active_ix)?;
            // add_container sets the active container to be the new container
            self.set_active_node(active_ix)?;
            let parent_ix = self.tree.parent_of(active_ix)?;
            self.layout(parent_ix);
        }
        self.validate();
        Ok(())
    }

    // Updates the tree's layout recursively starting from the active container.
    // If the active container is a view, it starts at the parent container.
    pub fn layout_active_of(&mut self, c_type: ContainerType) {
        if let Some(container_ix) = self.active_ix_of(c_type) {
            match c_type {
                ContainerType::Root |
                ContainerType::Output |
                ContainerType::Workspace => {
                    self.layout(container_ix);
                },
                ContainerType::Container => {
                    let mut fullscreen_apps = Vec::new();
                    let geometry = self.tree[container_ix].get_geometry()
                        .expect("Container didn't have a geometry");
                    self.layout_helper(container_ix, geometry, &mut fullscreen_apps);
                },
                ContainerType::View => {
                    warn!("Cannot simply update a view's geometry without {}",
                          "consulting container, updating it's parent");
                    self.layout_active_of(ContainerType::Container);
                }
            }
        } else {
            warn!("{:#?} did not have a parent of type {:?}, doing nothing!",
                   self, c_type);
        }
        self.validate();
    }

    /// Sets the active container to the given layout.
    ///
    /// If the container is a view, it sets the layout of its parent to the
    /// given layout.
    ///
    /// Automatically retiles the container whose layout was changed.
    pub fn set_active_layout(&mut self, new_layout: Layout) -> CommandResult {
        let mut node_ix = self.active_container
            .ok_or(TreeError::NoActiveContainer)?;
        if self.tree[node_ix].get_type() == ContainerType::View {
            node_ix = self.tree.parent_of(node_ix)
                .expect("View had no parent");
        }
        self.tree[node_ix].set_layout(new_layout)
            .map_err(TreeError::Container)?;
        self.validate();
        let workspace_ix = self.tree.ancestor_of_type(node_ix,
                                                      ContainerType::Workspace)?;
        self.layout(workspace_ix);
        Ok(())
    }

    /// Gets the active container and toggles it based on the following rules:
    /// * If horizontal, make it vertical
    /// * else, make it horizontal
    /// This method does *NOT* update the actual views geometry, that needs to be
    /// done separately by the caller
    pub fn toggle_cardinal_tiling(&mut self, id: Uuid) -> CommandResult {
        {
            // NOTE: This stupid mutable lookup can't be its own function, see:
            // https://www.reddit.com/r/rust/comments/55o54l/hey_rustaceans_got_an_easy_question_ask_here/d8pv5q9/?context=3
            let node_ix = try!(self.tree.lookup_id(id)
                               .ok_or(TreeError::NodeNotFound(id)));
            let container_t = self.tree[node_ix].get_type();
            if container_t == ContainerType::View {
                let parent_id = try!(self.parent_of(id)).get_id();
                return self.toggle_cardinal_tiling(parent_id)
            }
            let new_layout = match self.tree[node_ix].get_layout()? {
                Layout::Horizontal => Layout::Vertical,
                _ => Layout::Horizontal
            };
            self.set_layout(node_ix, new_layout)
        }
        self.validate();
        Ok(())
    }


    /// Calculates how much to scale on average for each value given.
    /// If the value is 0 (i.e the width or height of the container is 0),
    /// then it is calculated as max / children_values.len()
    fn calculate_scale(children_values: Vec<f32>, max: f32) -> f32 {
        let mut scale = 0.0;
        let len = children_values.len();
        for mut value in children_values {
            if value <= 0.0 {
                value = max / len.checked_sub(1).unwrap_or(1) as f32;
            }
            scale += value;
        }
        return scale;
    }

    fn generic_tile<SizeF, RemainF, PointF>
        (&mut self,
         node_ix: NodeIndex, geometry: Geometry, children: &[NodeIndex],
         new_size_f: SizeF, remaining_size_f: RemainF, new_point_f: PointF,
         fullscreen_apps: &mut Vec<NodeIndex>)
        where SizeF:   Fn(Size, Geometry) -> Size,
              RemainF: Fn(Geometry, Geometry) -> Size,
              PointF:  Fn(Size, Geometry) -> Point
    {
        let mut sub_geometry = geometry;
        for (index, child_ix) in children.iter().enumerate() {
            let child_size = self.tree[*child_ix].get_geometry()
                .expect("Child had no geometry").size;
            let new_size = new_size_f(child_size, sub_geometry);
            sub_geometry = Geometry {
                origin: sub_geometry.origin.clone(),
                size: new_size
            };
            // If last child, then just give it the remaining height
            if index == children.len() - 1 {
                let new_size = remaining_size_f(sub_geometry,
                                                self.tree[node_ix].get_geometry()
                                                .expect("Container had no geometry"));
                sub_geometry = Geometry {
                    origin: sub_geometry.origin,
                    size: new_size
                };
            }
            self.layout_helper(*child_ix, sub_geometry, fullscreen_apps);

            // Next sub container needs to start where this one ends
            let new_point = new_point_f(new_size, sub_geometry);
            sub_geometry = Geometry {
                // lambda to calculate new point, given a new size
                // which is calculated in the function
                origin: new_point,
                size: new_size
            };
        }
        self.validate();
    }

    pub fn set_layout(&mut self, node_ix: NodeIndex, new_layout: Layout) {
        match self.tree[node_ix] {
            Container::Container { ref mut layout , ref mut borders, .. } => {
                *layout = new_layout;
                // This needs to be done manually because
                // for some reason set_layout cannot be used.
                if let Some(ref mut _b) = borders.as_mut() {
                    _b.layout = Some(new_layout);
                }
            },
            ref container => {
                warn!("Can not set layout on non-container {:#?}", container);
                return;
            }
        }
        if new_layout == Layout::Vertical || new_layout == Layout::Horizontal {
            for child_ix in self.tree.children_of(node_ix) {
                self.normalize_container(child_ix).ok();
            }
            let workspace_ix = self.tree.ancestor_of_type(
                node_ix, ContainerType::Workspace)
                .expect("Node did not have a workspace as an ancestor");
            if self.tree.on_path(workspace_ix) {
                self.set_container_visibility(node_ix, true)
            }
        }
    }

    /// Normalizes the geometry of a view to be the same size as it's siblings,
    /// based on the parent container's layout, at the 0 point of the parent container.
    /// Note this does not auto-tile, only modifies this one view.
    ///
    /// Useful if a container's children want to be evenly distributed, or a new view
    /// is being added.
    pub fn normalize_view(&mut self, view: WlcView) -> CommandResult {
        if let Some(view_ix) = self.tree
            .descendant_with_handle(self.tree.root_ix(), view.into()) {
            match self.normalize_container(view_ix) {
                Ok(_) => Ok(()),
                Err(TreeError::ContainerWasFloating(node_ix)) => {
                    warn!("Node {:?} was floating! Not normalizing", node_ix);
                    Ok(())
                },
                err => err
            }
        } else {
            Err(TreeError::ViewNotFound(view))
        }
    }

    /// Normalizes the geometry of a view or a container of views so that
    /// the view is the same size as its siblings.
    pub fn normalize_container(&mut self, node_ix: NodeIndex) -> CommandResult {
        // if floating, do not normalize
        if self.tree[node_ix].floating() {
            return Err(TreeError::ContainerWasFloating(node_ix))
        }
        match self.tree[node_ix].get_type() {
            ContainerType::Container  => {
                for child_ix in self.tree.grounded_children(node_ix) {
                    self.normalize_container(child_ix).ok();
                }
            },
            ContainerType::View  => {
                let parent_ix = self.tree.ancestor_of_type(node_ix,
                                                        ContainerType::Container)
                    .expect("View had no container parent");
                let new_geometry: Geometry;
                let num_siblings = cmp::max(1, self.tree.grounded_children(parent_ix).len()
                                            .checked_sub(1).unwrap_or(0)) as u32;
                let parent_geometry = self.tree[parent_ix].get_actual_geometry()
                    .expect("Parent container had no geometry");
                match self.tree[parent_ix] {
                    Container::Container { ref layout, .. } => {
                        match *layout {
                            Layout::Horizontal => {
                                new_geometry = Geometry {
                                    origin: parent_geometry.origin.clone(),
                                    size: Size {
                                        w: parent_geometry.size.w / num_siblings,
                                        h: parent_geometry.size.h
                                    }
                                };
                            }
                            Layout::Vertical => {
                                new_geometry = Geometry {
                                    origin: parent_geometry.origin.clone(),
                                    size: Size {
                                        w: parent_geometry.size.w,
                                        h: parent_geometry.size.h / num_siblings
                                    }
                                };
                            },
                            Layout::Tabbed | Layout::Stacked =>
                                new_geometry = parent_geometry
                        }
                    },
                    _ => unreachable!()
                };
                self.tree[node_ix].set_geometry(ResizeEdge::empty(), new_geometry);
            },
            container => {
                error!("Tried to normalize a {:#?}", container);
                panic!("Can only normalize the view on a view or container")
            }
        }
        Ok(())
    }

    /// Tiles these containers above all the other containers in its workspace.
    ///
    /// If multiple containers are in the same workspace, each one will be drawn
    /// on top of the other, with the last one being the one ultimately seen by the user.
    ///
    /// # Panic
    /// This function will panic if the any of the containers are not a `View` or a `Container`
    pub fn layout_fullscreen_apps(&mut self, containers: Vec<NodeIndex>) {
        for node_ix in containers {
            let output_ix = self.tree.ancestor_of_type(node_ix, ContainerType::Output)
                .expect("Container did not have an output as an ancestor");
            let output_geometry = self.tree[output_ix].get_actual_geometry()
                .expect("Output did not have a geometry associated with it");

            // Sorry, this is an ugly borrow checker hack
            // Can't do self.layout() in Container::Container, borrowing mutably self mutably here.
            let maybe_node_ix = match self.tree[node_ix] {
                Container::View { handle, .. } => {
                    handle.set_geometry(ResizeEdge::empty(), output_geometry);
                    handle.bring_to_front();
                    let views = handle.get_output().get_views();
                    // TODO It would be nice to not have to iterate over
                    // all the views just to do this.
                    for view in views {
                        // make sure children render above fullscreen parent
                        if view.get_parent() == handle {
                            view.bring_to_front();
                        }
                    }
                    None
                },
                Container::Container { ref mut geometry, .. } => {
                    *geometry = output_geometry;
                    Some(node_ix)
                },
                ref container => {
                    error!("Expected a view or a container, got {:?}", container);
                    panic!("Expected a View or a Container, got something else");
                }
            };
            if let Some(node_ix) = maybe_node_ix {
                self.layout(node_ix);
            }
        }
    }

    /// Adds gaps between all the views of the container at the `NodeIndex`
    /// This does not recurse if a container is found.
    ///
    /// If the `NodeIndex` doesn't point to a `Container`, an error is returned.
    fn add_gaps(&mut self, node_ix: NodeIndex) -> CommandResult {
        let layout = match self.tree[node_ix] {
            Container::Container { layout, .. } => layout,
            _ => return Err(TreeError::UuidNotAssociatedWith(
                ContainerType::Container))
        };
        let gap = Borders::gap_size();
        if gap == 0 {
            return Ok(())
        }
        let children = self.tree.grounded_children(node_ix);
        for (index, child_ix) in children.iter().enumerate() {
            let child = &mut self.tree[*child_ix];
            match *child {
                Container::View { handle, .. } => {
                    let mut geometry = handle.get_geometry().unwrap();
                    geometry.origin.x += (gap / 2) as i32;
                    geometry.origin.y += (gap / 2) as i32;
                    if index == children.len() - 1 {
                        match layout {
                            Layout::Horizontal => {
                                geometry.size.w = geometry.size.w.saturating_sub(gap / 2)
                            },
                            Layout::Vertical => {
                                geometry.size.h = geometry.size.h.saturating_sub(gap / 2)
                            },
                            _ => {
                                // Nothing special for the last container,
                                // since only one is visible at a time
                            }
                        }
                    }
                    match layout {
                        Layout::Horizontal => {
                            geometry.size.w = geometry.size.w.saturating_sub(gap / 2);
                            geometry.size.h = geometry.size.h.saturating_sub(gap);
                        },
                        Layout::Vertical => {
                            geometry.size.w = geometry.size.w.saturating_sub(gap);
                            geometry.size.h = geometry.size.h.saturating_sub(gap / 2);
                        },
                        Layout::Tabbed | Layout::Stacked => {
                            geometry.size.w = geometry.size.w.saturating_sub(gap);
                            geometry.size.h = geometry.size.h.saturating_sub(gap)
                        }
                    }
                    handle.set_geometry(ResizeEdge::empty(), geometry);
                },
                // Do nothing, will get in the next recursion cycle
                Container::Container { .. } => {continue},
                ref container => {
                    error!("Iterating over a container, \
                            found non-view/containers!");
                    error!("Found: {:#?}", container);
                    panic!("Applying gaps, found a non-view/container")
                }
            }
        }
        Ok(())
    }

    /// Updates the geometry of the container, so that the borders are not
    /// hidden by the container. E.g this ensures that the borders are treated
    /// as part of the container for tiling/rendering purposes
    ///
    /// Returns the updated geometry for the container on success.
    /// That geometry should be used as the new constraint geometry for the
    /// children containers.
    fn update_container_geo_for_borders(&mut self, node_ix: NodeIndex,
                                        mut geometry: Geometry)
                                        -> Result<Geometry, TreeError> {
        // Stacked containers need bigger title bars.
        // This has to be done before borrowing self.tree to use the container
        let child_count = self.tree.children_of(node_ix).len();

        let container = &mut self.tree[node_ix];

        match *container {
            Container::Container { layout,
                                   ref mut apparent_geometry,
                                   geometry: ref mut actual_geometry,
                                   ref borders, .. } => {
                *actual_geometry = geometry;
                if borders.is_some() {
                    let title_count = match layout {
                        Layout::Stacked =>
                            // I don't understand why I have to use this
                            // instead of just child_count, but seems to work...
                            (2*child_count-1) as u32,
                        _ => 1
                    };
                    let gap = Borders::gap_size();
                    let thickness = Borders::thickness() + gap;
                    let edge_thickness = thickness / 2;
                    let title_size = Borders::title_bar_size();
                    let real_title_size = (title_size / 2) * title_count;
                    geometry.origin.y += edge_thickness as i32;
                    geometry.origin.y += real_title_size as i32;
                    geometry.size.h = geometry.size.h.saturating_sub(edge_thickness);
                    geometry.size.h = geometry.size.h.saturating_sub(real_title_size);
                }
                *apparent_geometry = geometry;
            },
            ref container => {
                error!("Attempted to add borders to non-view");
                error!("Found {:#?}", container);
                panic!("Applying gaps for borders, found non-view/container")
            }
        }
        Ok(geometry)
    }

    /// Updates the geometry of the view, so that the borders are not
    /// hidden by other views. E.g this ensures that the borders are treated
    /// as part of the container for tiling/rendering purposes
    fn update_view_geo_for_borders(&mut self, node_ix: NodeIndex) -> CommandResult {
        let container = &mut self.tree[node_ix];
        let mut geometry = container.get_geometry()
            .expect("Container had no geometry");
        match *container {
            Container::View { handle, .. } => {
                let thickness = Borders::thickness();
                if thickness == 0 {
                    return Ok(())
                }
                let edge_thickness = (thickness / 2) as i32;
                let title_size = Borders::title_bar_size();
                geometry.origin.x += edge_thickness;
                geometry.origin.y += edge_thickness;
                geometry.origin.y += title_size as i32;
                geometry.size.w = geometry.size.w.saturating_sub(thickness);
                geometry.size.h = geometry.size.h.saturating_sub(thickness);
                geometry.size.h = geometry.size.h.saturating_sub(title_size);
                handle.set_geometry(ResizeEdge::empty(), geometry);
            },
            ref container => {
                error!("Attempted to add borders to non-view");
                error!("Found {:#?}", container);
                panic!("Applying gaps for borders, found non-view/container")
            }
        }
        // Done to make the resizing on tiled works
        container.resize_borders(geometry);
        Ok(())
    }

    /// Draws the borders recursively, down from the top to the bottom.
    fn draw_borders_rec(&mut self, mut children: Vec<NodeIndex>)
                        -> CommandResult {
        let mut updated_nodes: HashSet<NodeIndex> = HashSet::from_iter(children.iter().cloned());

        while children.len() > 0 {
            let child_ix = children.pop().unwrap();

            for child in self.tree.grounded_children(child_ix) {
                if !updated_nodes.contains(&child) {
                    updated_nodes.insert(child);
                    children.push(child);
                }
            }

            let parent_ix = self.tree.parent_of(child_ix)
                .expect("Node had no parent");
            let children = self.tree.children_of(parent_ix);
            let index = children.iter().position(|&node_ix| node_ix == child_ix);
            let index_str = index.map(|num| (num + 1).to_string());

            // I can't do it inside the match ahead because it borrows
            // self.tree, and I need self.tree to get the children's titles
            let titles = children.iter()
                .filter(|&&node_ix| {
                    !self.tree[node_ix].floating()
                })
                .map(|&node_ix| {
                    match self.tree[node_ix] {
                        Container::Container { ref borders, .. } |
                        Container::View { ref borders, .. } =>
                            borders.as_ref().map( |b| b.title() )
                            .unwrap_or("").into(),
                        ref child => {
                            error!("Container child is {:#?}", child);
                            panic!("Container childr is not a Container nor a View")
                        }
                    }
                }).collect();

            if !self.tree.on_path(child_ix) && Some(child_ix) != self.active_container &&
                self.active_container.map(|active|
                                          self.tree.is_child_of(child_ix, active).ok() != Some(false) &&
                                          self.tree.is_child_of(active, child_ix).ok() == Some(false)) == Some(true) {
                self.set_borders(child_ix, borders::Mode::Inactive)?;
            } else {
                match self.tree[parent_ix] {
                    Container::Container { layout, ref mut borders, .. } => {
                        if layout == Layout::Tabbed || layout == Layout::Stacked {
                            borders.as_mut().map(|b| {

                                b.set_children(titles, index.expect("The node index was not a child"));

                                // Still necesary to draw the title when this
                                // container is inside another tabbed/stacked
                                b.set_title(format!("{:?} ({}/{})",
                                                    layout,
                                                    index_str.unwrap_or_else(|| "?".into()),
                                                    children.len()
                                ));
                            });
                        }
                    },
                    _ => {}
                }
                self.set_borders(child_ix, borders::Mode::Active)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::super::super::LayoutTree;

    #[test]
    /// Ensure that calculate_scale is fair to all it's children
    fn calculate_scale_test() {
        assert_eq!(LayoutTree::calculate_scale(vec!(), 0.0), 0.0);
        assert_eq!(LayoutTree::calculate_scale(vec!(5.0, 5.0, 5.0, 5.0, 5.0, 5.0), 0.0), 30.0);
        assert_eq!(LayoutTree::calculate_scale(vec!(5.0, 5.0, 5.0, 5.0, -5.0, 0.0), 5.0), 22.0);
    }
}
