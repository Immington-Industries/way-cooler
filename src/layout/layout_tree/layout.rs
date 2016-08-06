use std::cmp;

use petgraph::graph::NodeIndex;
use rustwlc::{Geometry, Point, Size, ResizeEdge};

use super::super::LayoutTree;
use super::super::container::{Container, ContainerType, Layout};

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
                let size = handle.get_resolution().clone();
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
                    size: handle.get_resolution().clone()
                };
                trace!("layout: Laying out workspace, using size of the screen output {:?}", handle);
                self.layout_helper(node_ix, output_geometry);
            }
            _ => {
                warn!("layout should not be called directly on a container, view\
                       laying out the entire tree just to be safe");
                let root_ix = self.tree.root_ix();
                self.layout(root_ix);
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
                trace!("layout_helper: Laying out entire tree");
                warn!("Ignoring geometry constraint ({:?}), \
                       deferring to each output's constraints",
                      geometry);
                for child_ix in self.tree.children_of(node_ix) {
                    self.layout(child_ix);
                }
            }
            ContainerType::Workspace => {
                {
                    let container_mut = self.tree.get_mut(node_ix).unwrap();
                    trace!("layout_helper: Laying out workspace {:?} with\
                            geometry constraints {:?}",
                        container_mut, geometry);
                    match *container_mut {
                        Container::Workspace { ref mut size, .. } => {
                            *size = geometry.size.clone();
                        }
                        _ => unreachable!()
                    };
                }
                for child_ix in self.tree.children_of(node_ix) {
                    self.layout_helper(child_ix, geometry.clone());
                }
            },
            ContainerType::Container => {
                {
                    let container_mut = self.tree.get_mut(node_ix).unwrap();
                    trace!("layout_helper: Laying out container {:?} with geometry constraints {:?}",
                           container_mut, geometry);
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
                        trace!("Layout was horizontal, laying out the sub-containers horizontally");
                        let children = self.tree.children_of(node_ix);
                        let mut scale = LayoutTree::calculate_scale(children.iter().map(|child_ix| {
                            let c_geometry = self.tree[*child_ix].get_geometry()
                                .expect("Child had no geometry");
                            c_geometry.size.w as f32
                        }).collect(), geometry.size.w as f32);

                        if scale > 0.1 {
                            scale = geometry.size.w as f32 / scale;
                            let new_size_f = |child_size: Size, sub_geometry: Geometry| {
                                Size {
                                    w: ((child_size.w as f32) * scale) as u32,
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
                            self.generic_tile(node_ix, geometry, scale, children,
                                              new_size_f, remaining_size_f, new_point_f);
                        }
                    }
                    Layout::Vertical => {
                        trace!("Layout was vertical, laying out the sub-containers vertically");
                        let children = self.tree.children_of(node_ix);
                        let mut scale = LayoutTree::calculate_scale(children.iter().map(|child_ix| {
                            let c_geometry = self.tree[*child_ix].get_geometry()
                                .expect("Child had no geometry");
                            c_geometry.size.h as f32
                        }).collect(), geometry.size.h as f32);

                        if scale > 0.1 {
                            scale = geometry.size.h as f32 / scale;
                            let new_size_f = |child_size: Size, sub_geometry: Geometry| {
                                Size {
                                    w: sub_geometry.size.w,
                                    h: ((child_size.h as f32) * scale) as u32
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
                            self.generic_tile(node_ix, geometry, scale, children,
                                              new_size_f, remaining_size_f, new_point_f);
                        }
                    }
                    Layout::Floating => {
                        trace!("Layout was floating, throwing the views on the screen {}",
                               "like I'm Jackson Pollock");
                    }
                    _ => unimplemented!()
                }
            }

            ContainerType::View => {
                let handle = match self.tree[node_ix] {
                    Container::View { ref handle, .. } => handle,
                    _ => unreachable!()
                };
                trace!("layout_helper: Laying out view {:?} with geometry constraints {:?}",
                       handle, geometry);
                handle.set_geometry(ResizeEdge::empty(), &geometry);
            }
        }
        self.validate();
    }


    // Updates the tree's layout recursively starting from the active container.
    // If the active container is a view, it starts at the parent container.
    pub fn layout_active_of(&mut self, c_type: ContainerType) {
        if let Some(container_ix) = self.active_ix_of(c_type) {
            match self.tree[container_ix].clone() {
                Container::Root { .. } |
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
            warn!("{:?} did not have a parent of type {:?}, doing nothing!",
                   self, c_type);
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
                value = max / cmp::max(1, len - 1) as f32;
            }
            scale += value;
        }
        return scale;
    }

    fn generic_tile<SizeF, RemainF, PointF>
        (&mut self,
         node_ix: NodeIndex, geometry: Geometry, scale: f32, children: Vec<NodeIndex>,
         new_size_f: SizeF, remaining_size_f: RemainF, new_point_f: PointF)
        where SizeF:   Fn(Size, Geometry) -> Size,
              RemainF: Fn(Geometry, Geometry) -> Size,
              PointF:  Fn(Size, Geometry) -> Point
    {
        trace!("Scaling factor: {:?}", scale);
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
        match  self.tree[node_ix] {
            Container::Container { ref mut layout, .. } => {
                *layout = new_layout;
            },
            ref container => {
                warn!("Can not set layout on non-container {:?}", container);
                return;
            }
        }
    }

    /// Normalizes the geometry of a view or a container of views so that
    /// the view is the same size as its siblings.
    ///
    /// See `normalize_view` for more information
    pub fn normalize_container(&mut self, node_ix: NodeIndex) {
        match self.tree[node_ix].get_type() {
            ContainerType::Container  => {
                for child_ix in self.tree.children_of(node_ix) {
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
                let num_siblings = cmp::max(1, self.tree.children_of(parent_ix).len() - 1)
                    as u32;
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
                            _ => unimplemented!()
                        }
                    },
                    _ => unreachable!()
                };
                trace!("Setting view {:?} to geometry: {:?}",
                    self.tree[node_ix], new_geometry);
                handle.set_geometry(ResizeEdge::empty(), &new_geometry);
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
