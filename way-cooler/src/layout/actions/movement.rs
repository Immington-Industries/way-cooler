use rustwlc::{Point, ResizeEdge};
use uuid::Uuid;
use petgraph::graph::NodeIndex;

use super::super::LayoutTree;
use super::super::commands::CommandResult;
use super::super::core::{Direction, ShiftDirection, TreeError};
use super::super::core::container::{Container, ContainerType, Handle, Layout};

#[derive(Debug, Clone)]
pub enum MovementError {
    /// Attempted to move the node behind the UUID in the given direction,
    /// which would cause it to leave its siblings.
    MoveOutsideSiblings(Uuid, Direction),
    /// There was a tree error, generally should abort operation and pass this
    /// up back through the caller.
    Tree(Box<TreeError>),
    /// Expected the view to be floating, but it was not
    NotFloating(NodeIndex)
}


impl LayoutTree {
    /// Will attempt to move the container at the UUID in the given direction.
    pub fn move_container(&mut self, uuid: Uuid, direction: Direction) -> CommandResult {
        let node_ix = try!(self.tree.lookup_id(uuid).ok_or(TreeError::NodeNotFound(uuid)));
        let old_parent_ix = try!(self.tree.parent_of(node_ix).map_err(|err| TreeError::PetGraph(err)));
        try!(self.move_recurse(node_ix, None, direction));
        if self.tree.can_remove_empty_parent(old_parent_ix) {
            self.remove_container(old_parent_ix);
        }
        self.validate();
        Ok(())
    }

    /// Returns the new parent of the active container if the move succeeds,
    /// Otherwise it signals what error occurred in the tree.
    pub fn move_recurse(&mut self, node_to_move: NodeIndex, move_ancestor: Option<NodeIndex>,
                           direction: Direction) -> Result<NodeIndex, TreeError> {
        match self.tree[node_to_move].get_type() {
            ContainerType::View | ContainerType::Container => { /* continue */ },
            _ => return Err(TreeError::UuidWrongType(self.tree[node_to_move].get_id(),
                                                     vec!(ContainerType::View,
                                                          ContainerType::Container)))
        }
        let parent_ix = try!(
            match move_ancestor {
                Some(node) => self.tree.parent_of(node),
                None => self.tree.parent_of(node_to_move)
            }.map_err(|err| TreeError::PetGraph(err)));
        match self.tree[parent_ix] {
            Container::Container { layout, .. } =>  {
                match (layout, direction) {
                    (Layout::Horizontal, Direction::Left) |
                    (Layout::Horizontal, Direction::Right) |
                    (Layout::Vertical, Direction::Up) |
                    (Layout::Vertical, Direction::Down) => {
                        if let Some(ancestor_ix) = move_ancestor {
                            match self.move_between_ancestors(node_to_move, ancestor_ix, direction) {
                                Ok(new_parent_ix) => Ok(new_parent_ix),
                                Err(MovementError::Tree(err)) => {
                                    error!("Tree error moving between ancestors: {:?}", err);
                                    Err(*err)
                                }
                                Err(MovementError::MoveOutsideSiblings(node, dir)) => {
                                    error!("Trying to move {:#?} in the {:?} direction somehow moved out of siblings",
                                           node, dir);
                                    panic!("Moving between ancestors failed in an unexpected way")
                                },
                                err => {
                                    error!("Unexpected error: {:#?}", err);
                                    panic!("unexpected error");
                                }
                            }
                        } else { /* Moving within current parent container */
                            match self.move_within_container(node_to_move, direction) {
                                Ok(new_parent_ix) => {
                                    Ok(new_parent_ix)
                                },
                                Err(MovementError::MoveOutsideSiblings(_,_)) => {
                                    self.move_recurse(node_to_move, Some(parent_ix), direction)
                                },
                                Err(MovementError::Tree(err)) => {
                                    Err(*err)
                                },
                                err => {
                                    error!("Unexpected error: {:#?}", err);
                                    panic!("unexpected error");
                                }
                            }
                        }
                    },
                    _ => { self.move_recurse(node_to_move, Some(parent_ix), direction) }
                }
            },
            Container::Workspace { .. } => {
                Err(TreeError::InvalidOperationOnRootContainer(self.tree[node_to_move].get_id()))
            }
            _ => unreachable!()
        }
    }

    /// Attempt to move a container at the node index in the given direction.
    ///
    /// If the node would move outside of its current container by moving in that
    /// direction, then MovementError::MoveOutsideSiblings is returned.
    /// If the tree state is invalid, an appropriate wrapped up error is returned.
    ///
    /// If successful, the parent index of the node is returned.
    fn move_within_container(&mut self, node_ix: NodeIndex, direction: Direction)
                             -> Result<NodeIndex, MovementError> {
        let parent_ix = try!(self.tree.parent_of(node_ix)
                             .map_err(|err| MovementError::Tree(
                                 Box::new(TreeError::PetGraph(err)))));
        let siblings_and_self = self.tree.children_of(parent_ix);
        let cur_index = try!(siblings_and_self.iter().position(|node| {
            *node == node_ix
        }).ok_or(MovementError::Tree(
            Box::new(TreeError::NodeNotFound(self.tree[node_ix].get_id())))));
        let maybe_new_index = match direction {
            Direction::Right | Direction::Down => {
                cur_index.checked_add(1)
            }
            Direction::Left  | Direction::Up => {
                cur_index.checked_sub(1)
            }
        };
        if maybe_new_index.is_some() && maybe_new_index.unwrap() < siblings_and_self.len() {
            // There is a sibling to swap with
            let swap_index = maybe_new_index.unwrap();
            let swap_ix = siblings_and_self[swap_index];
            match self.tree[swap_ix] {
                Container::View { .. } => {
                    try!(self.tree.swap_node_order(node_ix, swap_ix)
                         .map_err(|err| MovementError::Tree(
                             Box::new(TreeError::PetGraph(err)))))
                },
                Container::Container { .. } => {
                    try!(self.tree.move_into(node_ix, swap_ix)
                         .map_err(|err| MovementError::Tree(
                             Box::new(TreeError::PetGraph(err)))));
                    if let Some(handle) = self.tree[node_ix].get_handle() {
                        match handle {
                            Handle::View(view) => self.normalize_view(view),
                            _ => unreachable!()
                        }
                    }
                },
                _ => return Err(MovementError::Tree(
                    Box::new(TreeError::UuidWrongType(self.tree[swap_ix].get_id(),
                                             vec!(ContainerType::View, ContainerType::Container)))))
            };
            Ok(self.tree.parent_of(node_ix)
               .expect("Moved container had no new parent"))
        } else {
            // Tried to move outside the limit
            Err(MovementError::MoveOutsideSiblings(self.tree[node_ix].get_id(), direction))
        }
    }

    /// Moves the node in the direction, outside to ancestor siblings.
    ///
    /// Returns the new parent of the node on success
    ///
    /// This should only be called by the recursive function.
    fn move_between_ancestors(&mut self,
                              node_to_move: NodeIndex,
                              move_ancestor: NodeIndex,
                              direction: Direction)
                                 -> Result<NodeIndex, MovementError> {
        let cur_parent_ix = try!(self.tree.parent_of(move_ancestor)
                                 .map_err(|err| MovementError::Tree(
                                     Box::new(TreeError::PetGraph(err)))));
        let siblings_and_self = self.tree.children_of(cur_parent_ix);
        let cur_index = try!(siblings_and_self.iter().position(|node| {
            *node == move_ancestor
        }).ok_or(MovementError::Tree(
            Box::new(TreeError::NodeNotFound(self.tree[move_ancestor].get_id())))));
        let next_ix = match direction {
            Direction::Right | Direction::Down => {
                let next_index = cur_index + 1;
                if next_index as usize >= siblings_and_self.len() {
                    return self.tree.add_to_end(node_to_move,
                                                siblings_and_self[siblings_and_self.len() - 1],
                                                ShiftDirection::Left)
                        .and_then(|_| self.tree.parent_of(node_to_move))
                        .map_err(|err| MovementError::Tree(
                            Box::new(TreeError::PetGraph(err))))
                } else {
                    siblings_and_self[next_index]
                }
            },
            Direction::Left | Direction::Up => {
                if let Some(next_index) = cur_index.checked_sub(1) {
                    siblings_and_self[next_index]
                } else {
                    return self.tree.add_to_end(node_to_move,
                                                siblings_and_self[0],
                                                ShiftDirection::Right)
                        .and_then(|_| self.tree.parent_of(node_to_move))
                        .map_err(|err| MovementError::Tree(
                            Box::new(TreeError::PetGraph(err))))
                }
            }
        };
        // Replace ancestor location with the node we are moving,
        // shifts the others over
        let parent_ix = try!(match self.tree[next_ix] {
            Container::View { .. } => {
                match direction {
                    Direction::Right | Direction::Down => {
                        self.tree.place_node_at(node_to_move, next_ix, ShiftDirection::Left)
                    },
                    Direction::Left | Direction::Up => {
                        self.tree.place_node_at(node_to_move, next_ix, ShiftDirection::Right)
                    }
                }
            },
            Container::Container { .. } => {
                self.tree.move_into(node_to_move, next_ix)
            },
            _ => unreachable!()
        }.map_err(|err| MovementError::Tree(Box::new(TreeError::PetGraph(err)))));
        match self.tree[node_to_move] {
            Container::View { handle, .. } => {
                self.normalize_view(handle);
                Ok(parent_ix)
            },
            _ => {
                Err(MovementError::Tree(
                    Box::new(
                        TreeError::UuidWrongType(self.tree[node_to_move].get_id(),
                                                 vec!(ContainerType::View)))))
            },
        }
    }

    /// If the view behind the node index is floating, drags move it to a
    /// point on the screen.
    pub fn drag_floating(&mut self, node_ix: NodeIndex, point: Point, old_point: Point)
                         -> CommandResult {
        let container = &self.tree[node_ix];
        if !container.floating() {
            return Err(TreeError::Movement(MovementError::NotFloating(node_ix)))
        }
        match *container {
            Container::View { handle, .. } => {
                let dx = point.x - old_point.x;
                let dy = point.y - old_point.y;
                let mut geo = handle.get_geometry()
                    .expect("Could not get geometry of view");
                geo.origin.x += dx;
                geo.origin.y += dy;
                handle.set_geometry(ResizeEdge::empty(), geo);
                Ok(())
            },
            Container::Container { id, .. } | Container::Workspace { id, .. } |
            Container::Output { id, .. } | Container::Root(id) => {
                Err(TreeError::UuidWrongType(id, vec!(ContainerType::View)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::core::tree::tests::basic_tree;
    use super::super::super::{Direction, Container, ContainerType, Layout};
    use rustwlc::*;

    #[test]
    fn test_basic_move() {
        let mut tree = basic_tree();
        tree.add_view(WlcView::root()).unwrap();
        let active_uuid = tree.get_active_container().unwrap().get_id();
        let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
        let children = tree.tree.children_of(active_parent);
        assert_eq!(children[1], tree.active_container.unwrap());
        // These should do nothing, moving in wrong direction
        assert!(tree.move_container(active_uuid, Direction::Up).is_err());
        assert!(tree.move_container(active_uuid, Direction::Down).is_err());
        assert!(tree.move_container(active_uuid, Direction::Right).is_err());
        // test going left and right works
        assert!(tree.move_container(active_uuid, Direction::Left).is_ok());
        let children = tree.tree.children_of(active_parent);
        assert_eq!(children[0], tree.active_container.unwrap());
        assert!(tree.move_container(active_uuid, Direction::Right).is_ok());
        let children = tree.tree.children_of(active_parent);
        assert_eq!(children[1], tree.active_container.unwrap());
        // test going up and down works
        tree.toggle_cardinal_tiling();
        assert!(tree.move_container(active_uuid, Direction::Up).is_ok());
        let children = tree.tree.children_of(active_parent);
        assert_eq!(children[0], tree.active_container.unwrap());
        assert!(tree.move_container(active_uuid, Direction::Down).is_ok());
        let children = tree.tree.children_of(active_parent);
        assert_eq!(children[1], tree.active_container.unwrap());
    }

    #[test]
    fn test_move_into_sub_container_dif_layout() {
        let mut tree = basic_tree();
        tree.switch_to_workspace("2");
        let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
        let children = tree.tree.children_of(active_parent);
        assert_eq!(Some(children[0]), tree.active_container);
        // make the first view have a vertical layout
        tree.toggle_active_layout(Layout::Vertical).unwrap();
        tree.active_container = Some(children[1]);
        let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
        let children = tree.tree.children_of(active_parent);
        let active_uuid = tree.get_active_container().unwrap().get_id();
        // make sure the first container is the sub container, second is the view
        assert_eq!(tree.tree[children[0]].get_type(), ContainerType::Container);
        assert_eq!(tree.tree[children[1]].get_type(), ContainerType::View);
        assert!(tree.move_container(active_uuid, Direction::Left).is_ok());
        let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
        let children = tree.tree.children_of(active_parent);
        // we should all be in the same container now, in the vertical one
        assert_eq!(children.len(), 2);
        match tree.tree[active_parent] {
            Container::Container { ref layout, .. } => {
                assert_eq!(*layout, Layout::Vertical);
            }
            _ => panic!("Parent of active was not a vertical container")
        }
    }

    #[test]
    fn test_move_into_sub_container_same_layout() {
        let mut tree = basic_tree();
        tree.switch_to_workspace("2");
        let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
        let children = tree.tree.children_of(active_parent);
        assert_eq!(Some(children[0]), tree.active_container);
        // make the first view have a vertical layout
        tree.toggle_active_layout(Layout::Horizontal).unwrap();
        let horizontal_id = tree.tree[tree.tree.parent_of(tree.active_container.unwrap()).unwrap()].get_id();
        tree.active_container = Some(children[1]);
        let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
        let children = tree.tree.children_of(active_parent);
        let active_uuid = tree.get_active_container().unwrap().get_id();
        // make sure the first container is the sub container, second is the view
        assert_eq!(tree.tree[children[0]].get_type(), ContainerType::Container);
        assert_eq!(tree.tree[children[1]].get_type(), ContainerType::View);
        assert!(tree.move_container(active_uuid, Direction::Left).is_ok());
        let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
        let children = tree.tree.children_of(active_parent);
        // we should all be in the same container now, in the sub horizontal one
        assert_eq!(children.len(), 2);
        match tree.tree[active_parent] {
            Container::Container { ref layout, ref id, .. } => {
                assert_eq!(*layout, Layout::Horizontal);
                assert_eq!(*id, horizontal_id);
            }
            _ => panic!("Parent of active was not a vertical container")
        }
    }

    #[test]
    fn test_move_against_edges() {
        let mut tree = basic_tree();
        tree.switch_to_workspace("2");
        // move the containers into one sub-vertical container, so we can test moving
        // to the right and left outside this container
        {
            let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
            let children = tree.tree.children_of(active_parent);
            assert_eq!(Some(children[0]), tree.active_container);
            // make the first view have a vertical layout
            tree.toggle_active_layout(Layout::Horizontal).unwrap();
            tree.active_container = Some(children[1]);
            let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
            let children = tree.tree.children_of(active_parent);
            let active_uuid = tree.get_active_container().unwrap().get_id();
            // make sure the first container is the sub container, second is the view
            assert_eq!(tree.tree[children[0]].get_type(), ContainerType::Container);
            assert_eq!(tree.tree[children[1]].get_type(), ContainerType::View);
            assert!(tree.move_container(active_uuid, Direction::Left).is_ok());
        }
        let active_ix = tree.active_container.unwrap();
        let active_id = tree.tree[active_ix].get_id();
        let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
        let children = tree.tree.children_of(active_parent);
        assert_eq!(Some(children[1]), tree.active_container);
        assert!(tree.move_container(active_id, Direction::Right).is_ok());
        let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
        let children = tree.tree.children_of(active_parent);
        // Should only be the moved child here and the vertical container
        assert_eq!(tree.tree[children[0]].get_type(), ContainerType::Container);
        assert_eq!(tree.tree[children[1]].get_type(), ContainerType::View);

        // move it back
        assert!(tree.move_container(active_id, Direction::Left).is_ok());
        let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
        let children = tree.tree.children_of(active_parent);
        assert_eq!(tree.tree[children[0]].get_type(), ContainerType::View);
        assert_eq!(tree.tree[children[1]].get_type(), ContainerType::View);


        // Do it to the left now
        assert!(tree.move_container(active_id, Direction::Left).is_ok());
        assert!(tree.move_container(active_id, Direction::Left).is_ok());
        let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
        let children = tree.tree.children_of(active_parent);

        // Should only be the moved child here and the vertical container
        assert_eq!(tree.tree[children[0]].get_type(), ContainerType::View);
        assert_eq!(tree.tree[children[1]].get_type(), ContainerType::Container);
        assert!(tree.move_container(active_id, Direction::Right).is_ok());
        let active_parent = tree.tree.parent_of(tree.active_container.unwrap()).unwrap();
        let children = tree.tree.children_of(active_parent);
        assert_eq!(tree.tree[children[0]].get_type(), ContainerType::View);
        assert_eq!(tree.tree[children[1]].get_type(), ContainerType::View);
    }
}
