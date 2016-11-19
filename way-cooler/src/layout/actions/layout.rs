use std::cmp;

use petgraph::graph::NodeIndex;
use rustwlc::{Geometry, Point, Size, ResizeEdge};

use super::super::{LayoutTree, TreeError};
use super::super::commands::CommandResult;
use super::super::core::container::{Container, ContainerType, Layout};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
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
                let handle = match self.tree[node_ix] {
                    Container::Output { ref handle, .. } => handle.clone(),
                    _ => unreachable!()
                };
                let size = handle.get_resolution()
                    .expect("Couldn't get resolution");
                let geometry = Geometry {
                    origin: Point { x: 0, y: 0 },
                    size: size
                };
                for workspace_ix in self.tree.children_of(node_ix) {
                    self.layout_helper(workspace_ix, geometry.clone());
                }
            }
            ContainerType::Workspace => {
                // get geometry from the parent output
                let output_ix = self.tree.ancestor_of_type(node_ix, ContainerType::Output)
                    .expect("Workspace had no output parent");
                let handle = match self.tree[output_ix] {
                    Container::Output{ ref handle, .. } => handle.clone(),
                    _ => unreachable!()
                };
                let output_geometry = Geometry {
                    origin: Point { x: 0, y: 0},
                    size: handle.get_resolution()
                        .expect("Couldn't get resolution")
                };
                self.layout_helper(node_ix, output_geometry);
            }
            ContainerType::Container => {
                let geometry = match self.tree[node_ix] {
                    Container::Container { geometry, .. } => geometry,
                    _ => unreachable!()
                };
                self.layout_helper(node_ix, geometry);
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
    fn layout_helper(&mut self, node_ix: NodeIndex, geometry: Geometry) {
        match self.tree[node_ix].get_type() {
            ContainerType::Root | ContainerType::Output => {
                warn!("Ignoring geometry constraint ({:#?}), \
                       deferring to each output's constraints",
                      geometry);
                for child_ix in self.tree.children_of(node_ix) {
                    self.layout(child_ix);
                }
            }
            ContainerType::Workspace => {
                {
                    let container_mut = self.tree.get_mut(node_ix).unwrap();
                    match *container_mut {
                        Container::Workspace { ref mut size, .. } => {
                            *size = geometry.size.clone();
                        }
                        _ => unreachable!()
                    };
                }
                for child_ix in self.tree.grounded_children(node_ix) {
                    self.layout_helper(child_ix, geometry.clone());
                }
                // place floating children above everything else
                let root_ix = self.tree.children_of(node_ix)[0];
                for child_ix in self.tree.floating_children(root_ix) {
                    self.place_floating(child_ix);
                }
            },
            ContainerType::Container => {
                {
                    let container_mut = self.tree.get_mut(node_ix).unwrap();
                    match *container_mut {
                        Container::Container { geometry: ref mut c_geometry, .. } => {
                            *c_geometry = geometry.clone();
                        },
                        _ => unreachable!()
                    };
                }
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
                                    cur_geometry.origin.x as u32 + cur_geometry.size.w -
                                    sub_geometry.origin.x as u32;
                                Size {
                                    w: remaining_width,
                                    h: sub_geometry.size.h
                                }
                            };
                            let new_point_f = |new_size: Size, sub_geometry: Geometry| {
                                Point {
                                    x: sub_geometry.origin.x + new_size.w as i32,
                                    y: sub_geometry.origin.y
                                }
                            };
                            self.generic_tile(node_ix, geometry, children,
                                              new_size_f, remaining_size_f, new_point_f);
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
                                    cur_geometry.origin.y as u32 + cur_geometry.size.h -
                                    sub_geometry.origin.y as u32;
                                Size {
                                    w: sub_geometry.size.w,
                                    h: remaining_height
                                }
                            };
                            let new_point_f = |new_size: Size, sub_geometry: Geometry| {
                                Point {
                                    x: sub_geometry.origin.x,
                                    y: sub_geometry.origin.y + new_size.h as i32
                                }
                            };
                            self.generic_tile(node_ix, geometry, children,
                                              new_size_f, remaining_size_f, new_point_f);
                        }
                    }
                }
            }

            ContainerType::View => {
                let handle = match self.tree[node_ix] {
                    Container::View { ref handle, .. } => handle,
                    _ => unreachable!()
                };
                handle.set_geometry(ResizeEdge::empty(), geometry);
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
            match *container {
                Container::View { handle, .. } => {
                    handle.set_geometry(ResizeEdge::empty(), new_geometry);
                },
                Container::Container { ref mut geometry, .. } => {
                    *geometry = new_geometry
                },
                _ => return Err(TreeError::UuidWrongType(id, vec!(ContainerType::View,
                                                                  ContainerType::Container)))
            }
        }
        let root_ix = self.tree.root_ix();
        let root_c_ix = try!(self.tree.follow_path_until(root_ix, ContainerType::Container)
                             .map_err(|_| TreeError::NoActiveContainer));
        let next_ix = self.tree.next_sibling(node_ix)
            .unwrap_or_else(|| self.tree.parent_of(node_ix).expect("node_ix had no parent!"));
        try!(self.tree.move_into(node_ix, root_c_ix)
             .map_err(|err| TreeError::PetGraph(err)));
        if !self.tree[next_ix].floating() {
            self.tree.set_ancestor_paths_active(next_ix);
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
        self.normalize_container(node_ix);
        let root_ix = self.tree.root_ix();
        let root_c_ix = try!(self.tree.follow_path_until(root_ix, ContainerType::Container)
                             .map_err(|_| TreeError::NoActiveContainer));
        let parent_ix = self.tree.parent_of(root_c_ix).unwrap();
        self.layout(parent_ix);
        Ok(())
    }

    /// If the node is floating, places it at its reported position, above all
    /// other nodes.
    fn place_floating(&mut self, node_ix: NodeIndex) {
        if !self.tree[node_ix].floating() {
            // This could mess up the layout very badly, that's why it's an error
            error!("Tried to absolutely place a non-floating view!");
            return
        }
        match self.tree[node_ix] {
            Container::Container { .. } => { unimplemented!() },
            Container::View { ref handle, .. } => {
                handle.bring_to_front();
            },
            _ => unreachable!()
        }
        for child_ix in self.tree.floating_children(node_ix) {
            self.place_floating(child_ix);
        }
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
                self.set_layout(parent_ix, new_layout);
                return Ok(())
            }

            let active_geometry = self.get_active_container()
                .expect("Could not get the active container")
                .get_geometry().expect("Active container had no geometry");

            let mut new_container = Container::new_container(active_geometry);
            new_container.set_layout(new_layout).ok();
            try!(self.add_container(new_container, active_ix));
            // add_container sets the active container to be the new container
            try!(self.set_active_node(active_ix));
        }
        self.validate();
        Ok(())
    }

    // Updates the tree's layout recursively starting from the active container.
    // If the active container is a view, it starts at the parent container.
    pub fn layout_active_of(&mut self, c_type: ContainerType) {
        if let Some(container_ix) = self.active_ix_of(c_type) {
            match self.tree[container_ix].clone() {
                Container::Root(_)  |
                Container::Output { .. } |
                Container::Workspace { .. } => {
                    self.layout(container_ix);
                }
                Container::Container { ref geometry, .. } => {
                    self.layout_helper(container_ix, geometry.clone());
                },
                Container::View { .. } => {
                    warn!("Cannot simply update a view's geometry without {}",
                          "consulting container, updating it's parent");
                    self.layout_active_of(ContainerType::Container);
                },

            }
        } else {
            warn!("{:#?} did not have a parent of type {:?}, doing nothing!",
                   self, c_type);
        }
        self.validate();
    }

    /// Gets the active container and toggles it based on the following rules:
    /// * If horizontal, make it vertical
    /// * else, make it horizontal
    /// This method does *NOT* update the actual views geometry, that needs to be
    /// done separately by the caller
    pub fn toggle_cardinal_tiling(&mut self) {
        if let Some(active_ix) = self.active_ix_of(ContainerType::Container) {
            match self.tree[active_ix] {
                Container::Container { ref mut layout, .. } => {
                    match *layout {
                        Layout::Horizontal => {
                            *layout = Layout::Vertical
                        }
                        _ => {
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
         node_ix: NodeIndex, geometry: Geometry, children: Vec<NodeIndex>,
         new_size_f: SizeF, remaining_size_f: RemainF, new_point_f: PointF)
        where SizeF:   Fn(Size, Geometry) -> Size,
              RemainF: Fn(Geometry, Geometry) -> Size,
              PointF:  Fn(Size, Geometry) -> Point
    {
        let mut sub_geometry = geometry.clone();
        for (index, child_ix) in children.iter().enumerate() {
            let child_size: Size;
            {
                let child = &self.tree[*child_ix];
                child_size = child.get_geometry()
                    .expect("Child had no geometry").size;
            }
            let new_size = new_size_f(child_size, sub_geometry.clone());
            sub_geometry = Geometry {
                origin: sub_geometry.origin.clone(),
                size: new_size.clone()
            };
            // If last child, then just give it the remaining height
            if index == children.len() - 1 {
                let new_size = remaining_size_f(sub_geometry.clone(),
                                                self.tree[node_ix].get_geometry()
                                                .expect("Container had no geometry"));
                sub_geometry = Geometry {
                    origin: sub_geometry.origin,
                    size: new_size
                };
            }
            self.layout_helper(*child_ix, sub_geometry.clone());

            // Next sub container needs to start where this one ends
            let new_point = new_point_f(new_size.clone(), sub_geometry.clone());
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
            Container::Container { ref mut layout, .. } => {
                *layout = new_layout;
            },
            ref container => {
                warn!("Can not set layout on non-container {:#?}", container);
                return;
            }
        }
    }

    /// Normalizes the geometry of a view or a container of views so that
    /// the view is the same size as its siblings.
    ///
    /// See `normalize_view` for more information
    pub fn normalize_container(&mut self, node_ix: NodeIndex) {
        // if floating, do not normalize
        if self.tree[node_ix].floating() {
            if cfg!(debug_assertions) {
                error!("Tried to normalize {:?}\n{:#?}", node_ix, self);
                panic!("Tried to normalize a floating view, are you sure you want to do that?")
            } else {
                warn!("Tried to normalize {:?}\n{:#?}", node_ix, self);
                return
            }
        }
        match self.tree[node_ix].get_type() {
            ContainerType::Container  => {
                for child_ix in self.tree.grounded_children(node_ix) {
                    self.normalize_container(child_ix)
                }
            },
            ContainerType::View  => {
                let handle = match self.tree[node_ix] {
                    Container::View { ref handle, .. } => handle.clone(),
                    _ => unreachable!()
                };
                let parent_ix = self.tree.ancestor_of_type(node_ix,
                                                        ContainerType::Container)
                    .expect("View had no container parent");
                let new_geometry: Geometry;
                let num_siblings = cmp::max(1, self.tree.grounded_children(parent_ix).len()
                                            .checked_sub(1).unwrap_or(0)) as u32;
                let parent_geometry = self.tree[parent_ix].get_geometry()
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
                            }
                        }
                    },
                    _ => unreachable!()
                };
                handle.set_geometry(ResizeEdge::empty(), new_geometry);
            },
            _ => panic!("Can only normalize the view on a view or container")
        }
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
