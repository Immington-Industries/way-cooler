//! A tree represented via a petgraph graph, used for way-cooler's
//! layout.

use std::iter::Iterator;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::fmt::{Debug, Formatter};
use std::fmt::Result as FmtResult;

use petgraph::EdgeDirection;
use petgraph::graph::{Graph, NodeIndex, EdgeIndex};
use uuid::Uuid;

use rustwlc::WlcView;

use super::path::{Path, PathBuilder};

use layout::{Container, ContainerType, Handle};

#[derive(Clone, Copy, Debug)]
pub enum GraphError {
    /// These nodes were not siblings.
    NotSiblings(NodeIndex, NodeIndex),
    /// This node had no parent
    NoParent(NodeIndex),
    /// A node could not be found in the tree with this type.
    /// Gives the node where the search was started
    NotFound(ContainerType, NodeIndex)
}

/// Layout tree implemented with petgraph.
pub struct InnerTree {
    graph: Graph<Container, Path>, // Directed graph
    id_map: HashMap<Uuid, NodeIndex>,
    view_map: HashMap<WlcView, NodeIndex>,
    root: NodeIndex
}

/// The direction to shift sibling nodes when doing a tree transformation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShiftDirection {
    Left,
    Right
}

impl Debug for InnerTree {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        let active_path: String = self.active_path().iter().fold("0".into(), |acc, &(node_ix, _)| {
            format!("{} -> {}", acc, node_ix.index())
        });
        f.debug_struct("InnerTree")
            .field("graph", &self.graph as &Debug)
            .field("id_map", &self.id_map as &Debug)
            .field("view_map", &self.view_map as &Debug)
            .field("root", &self.root as &Debug)
            .field("active_path", &active_path as &Debug)
            .finish()
    }
}

impl InnerTree {
    /// Creates a new layout tree with a root node.
    pub fn new() -> InnerTree {
        let mut graph = Graph::new();
        let root_ix = graph.add_node(Container::new_root());
        InnerTree {
            graph: graph,
            id_map: HashMap::new(),
            view_map: HashMap::new(),
            root: root_ix
        }
    }

    /// Determines if the container at the node index is the root.
    /// Normally, this should only be true if the NodeIndex value is 1.
    pub fn is_root_container(&self, node_ix: NodeIndex) -> bool {
        if let Ok(parent_ix) = self.parent_of(node_ix) {
            self.graph[parent_ix].get_type() == ContainerType::Workspace
        } else {
            false
        }
    }


    /// Gets the index of the tree's root node
    pub fn root_ix(&self) -> NodeIndex {
        self.root
    }

    /// Gets the active path, starting at the root node.
    pub fn active_path(&self) -> Vec<(NodeIndex, &Path)> {
        let mut result = Vec::with_capacity(self.graph.raw_edges().len());
        let mut next_ix = Some(self.root);
        while let Some(cur_ix) = next_ix {
            let maybe_edge = self.graph.edges_directed(cur_ix, EdgeDirection::Outgoing)
                .find(|&(_, weight): &(_, &Path)| weight.active);
            if let Some(edge) = maybe_edge {
                result.push(edge);
                next_ix = Some(edge.0);
            } else {
                next_ix = None;
            }
        }
        result
    }

    /// Follows the active path beneath the node until it ends.
    /// Returns the last node in the chain.
    pub fn follow_path(&self, node_ix: NodeIndex) -> NodeIndex {
        let mut next_ix = Some(node_ix);
        while let Some(cur_ix) = next_ix {
            let maybe_edge = self.graph.edges_directed(cur_ix, EdgeDirection::Outgoing)
                .find(|&(_, weight): &(_, &Path)| weight.active);
            if let Some(edge) = maybe_edge {
                next_ix = Some(edge.0);
            } else {
                return cur_ix
            }
        }
        node_ix
    }

    /// Follows the active path beneath the node until a container with the
    /// given type is found, or the path ends. If the path ends, the last node
    /// found is returned.
    ///
    /// Note that if there is no active path beneath the start node, that node
    /// is the node that is returned in the error.
    pub fn follow_path_until(&self, node_ix: NodeIndex, c_type: ContainerType) -> Result<NodeIndex, NodeIndex> {
        let mut next_ix = Some(node_ix);
        while let Some(cur_ix) = next_ix {
            if self[cur_ix].get_type() == c_type {
                return Ok(cur_ix);
            }
            let maybe_edge = self.graph.edges_directed(cur_ix, EdgeDirection::Outgoing)
                .find(|&(_, weight): &(_, &Path)| weight.active);
            if let Some(edge) = maybe_edge {
                next_ix = Some(edge.0);
            } else {
                return Err(cur_ix)
            }
        }
        Err(node_ix)
    }

    /// Gets the weight of a possible edge between two notes
    pub fn get_edge_weight_between(&self, parent_ix: NodeIndex,
                                   child_ix: NodeIndex) -> Option<&Path> {
        self.graph.find_edge(parent_ix, child_ix)
            .and_then(|edge_ix| self.graph.edge_weight(edge_ix))
    }

    /// Gets the edge value of the largest child of the node
    fn largest_child(&self, node: NodeIndex) -> (NodeIndex, Path) {
        use std::cmp::{Ord, Ordering};
        self.graph.edges_directed(node, EdgeDirection::Outgoing)
            .fold((node, Path::zero()), |(old_node, old_edge), (new_node, new_edge)|
                  match <u32 as Ord>::cmp(&old_edge, new_edge) {
                      Ordering::Less => (new_node, *new_edge),
                      Ordering::Greater => (old_node, old_edge),
                      Ordering::Equal =>
                          panic!("largest_child: Node {:?} had two equal children {:#?} and {:#?}",
                          node, old_edge, new_edge)
                  })
    }

    /// Adds a new child to a node at the index, returning the node index
    /// of the new child node.
    ///
    /// If active is true, the path to the new node is made active.
    pub fn add_child(&mut self, parent_ix: NodeIndex, val: Container, active: bool) -> NodeIndex {
        let id = val.get_id();
        let maybe_view = match val.get_handle() {
            Some(Handle::View(view)) => Some(view),
            _ => None
        };
        let child_ix = self.graph.add_node(val);
        let edge = self.attach_child(parent_ix, child_ix);
        if active {
            self.set_ancestor_paths_active(child_ix);
        } else {
            let mut weight = self.graph.edge_weight_mut(edge)
                .expect("Could not get edge weight of parent/child");
            weight.active = false;
        }
        self.id_map.insert(id, child_ix);
        if let Some(view) = maybe_view {
            self.view_map.insert(view, child_ix);
        }
        child_ix
    }

    /// Add an existing node (detached in the graph) to the tree.
    /// Note that floating nodes shouldn't exist for too long.
    fn attach_child(&mut self, parent_ix: NodeIndex, child_ix: NodeIndex) -> EdgeIndex {
        if self.has_parent(child_ix) {
            panic!("attach_child: child had a parent!")
        }

        let parent_type = self.graph.node_weight(parent_ix)
            .expect("attach_child: parent not found").get_type();
        let child_type = self.graph.node_weight(child_ix)
            .expect("attach_child: child not found").get_type();

        if !parent_type.can_have_child(child_type) {
            panic!("Attempted to give a {:?} a {:?} child!",
                   parent_type, child_type);
        }
        let (_ix, mut biggest_child) = self.largest_child(parent_ix);
        *biggest_child += 1;
        let result = self.graph.update_edge(parent_ix, child_ix, biggest_child);
        self.normalize_edge_weights(parent_ix);
        result
    }

    /// Finds the index of the container at the child index's parent,
    /// modifies it so that it's the given child number in the list.
    pub fn set_child_pos(&mut self, child_ix: NodeIndex, mut child_pos: u32) {
        let parent_ix = self.parent_of(child_ix)
            .expect("Child had no parent");
        let siblings = self.children_of(parent_ix);
        if child_pos > siblings.len() as u32 {
            child_pos = siblings.len() as u32;
        }
        let mut counter = child_pos + 1;
        for sibling_ix in siblings {
            let mut edge_weight = *self.get_edge_weight_between(parent_ix, sibling_ix)
                .expect("Sibling had no edge weight");
            if *edge_weight < child_pos {
                continue;
            }
            *edge_weight = counter;
            self.graph.update_edge(parent_ix, sibling_ix, edge_weight);
            counter += 1;
        }
        let last_pos = PathBuilder::new(child_pos).active(true).build();
        self.graph.update_edge(parent_ix, child_ix, last_pos);
        self.normalize_edge_weights(parent_ix);
    }

    /// Swaps the edge weight of the two child nodes. The nodes must
    /// be siblings of each other, otherwise this function will fail.
    pub fn swap_node_order(&mut self, child1_ix: NodeIndex,
                           child2_ix: NodeIndex) -> Result<(), GraphError> {
        let parent1_ix = try!(self.parent_of(child1_ix));
        let parent2_ix = try!(self.parent_of(child2_ix));
        if parent2_ix != parent1_ix {
            return Err(GraphError::NotSiblings(child1_ix, child2_ix))
        }
        let parent_ix = parent2_ix;
        let mut child1_weight = *self.get_edge_weight_between(parent_ix, child1_ix)
            .expect("Could not get weight between parent and child");
        let mut child2_weight = *self.get_edge_weight_between(parent_ix, child2_ix)
            .expect("Could not get weight between parent and child");
        // We don't want to swap the active flag
        child1_weight.active = child1_weight.active ^ child2_weight.active;
        child2_weight.active = child1_weight.active ^ child2_weight.active;
        child1_weight.active = child1_weight.active ^ child2_weight.active;
        self.graph.update_edge(parent_ix, child1_ix, child2_weight);
        self.graph.update_edge(parent_ix, child2_ix, child1_weight);
        self.normalize_edge_weights(parent1_ix);
        Ok(())
    }

    /// Moves the node index at source so that it is a child of the target node.
    /// If the node was moved, the new parent of the source node is returned
    /// (which is always the same as the target node).
    ///
    /// If the source node is not floating, then the new connection is made active.
    pub fn move_into(&mut self, source: NodeIndex, target: NodeIndex)
                     -> Result<NodeIndex, GraphError> {
        let source_parent = try!(self.parent_of(source));
        let source_parent_edge = self.graph.find_edge(source_parent, source)
            .expect("Source node and it's parent were not linked");
        self.graph.remove_edge(source_parent_edge);
        let mut highest_weight = self.graph.edges_directed(target, EdgeDirection::Outgoing)
            .map(|(_ix, weight)| *weight).max()
            .unwrap_or(PathBuilder::new(0).build());
        highest_weight.weight = *highest_weight + 1;
        self.graph.update_edge(target, source, highest_weight);
        if !self[source].floating() {
            self.set_ancestor_paths_active(source);
        }
        self.normalize_edge_weights(source_parent);
        self.normalize_edge_weights(target);
        Ok(target)
    }

    /// Places the source node at the position where the target node is.
    ///
    /// Each node that has a weight >= the source's new weight is shifted over 1
    ///
    /// If the operation succeeds, the source's new parent (the target parent) is returned.
    pub fn place_node_at(&mut self, source: NodeIndex, target: NodeIndex, dir: ShiftDirection)
                         -> Result<NodeIndex, GraphError> {
        trace!("Placing source {:?} at target {:?}. Shifting to {:?}",
               source, target, dir);
        let target_parent = try!(self.parent_of(target));
        let target_parent_edge = self.graph.find_edge(target_parent, target)
            .expect("Target node and it's parent were not linked");
        let target_weight = match dir {
            ShiftDirection::Left => {
                self.graph.edge_weight(target_parent_edge).map(|weight| *weight)
            },
            ShiftDirection::Right=> {
                self.graph.edge_weight(target_parent_edge).map(|weight| {
                    let mut new_weight = *weight;
                    *new_weight = *new_weight + 1;
                    new_weight.active = true;
                    new_weight
                })
            }
        }.expect("Could not get the weight of the edge between target and parent");
        let bigger_target_siblings: Vec<NodeIndex> = self.graph.edges_directed(target_parent, EdgeDirection::Outgoing)
            .filter(|&(_node_ix, sibling_weight)| *sibling_weight >= target_weight)
            .map(|(node_ix, _)| node_ix).collect();
        let source_parent = try!(self.parent_of(source));
        let source_parent_edge = self.graph.find_edge(source_parent, source)
            .expect("Source node and it's parent were not linked");
        for sibling_ix in bigger_target_siblings {
            let sibling_edge = self.graph.find_edge(target_parent, sibling_ix)
                .expect("Sibling to target was not linked to target's parent");
            let weight = self.graph.edge_weight_mut(sibling_edge)
                .expect("Could not get the weight of the edge between target sibling and target parent");
            trace!("Sibling {:?} previously had an edge weight of {:?} to {:?}", sibling_ix, weight, target_parent);
            **weight = **weight + 1;
            weight.active = false;
            trace!("Sibling {:?}, edge weight to {:?} is now {:?}", sibling_ix, target_parent, weight);
        }
        trace!("Removing edge between child {:?} and parent {:?}", source, source_parent);
        self.graph.remove_edge(source_parent_edge);
        trace!("Adding edge between child {:?} and parent {:?} w/ weight {:?}", source, target_parent, target_weight);
        self.graph.update_edge(target_parent, source, target_weight);
        self.normalize_edge_weights(target_parent);
        self.normalize_edge_weights(source_parent);
        Ok(target_parent)
    }


    /// Adds the source node to the end of the target's siblings.
    /// If dir is Left, then it is added to the right (all shifted left) and vice versa.
    ///
    /// Returns the new parent of the source after the transformation, if no error occurred.
    pub fn add_to_end(&mut self, source: NodeIndex, target: NodeIndex, dir: ShiftDirection)
                      -> Result<NodeIndex, GraphError> {
        let target_parent = try!(self.parent_of(target));
        let siblings = self.children_of(target_parent);
        let source_parent = try!(self.parent_of(source));
        let source_parent_edge = self.graph.find_edge(source_parent, source)
            .expect("Source node and it's parent were not linked");
        match dir {
            ShiftDirection::Left => {
                trace!("place_node edge case: placing in the last place of the sibling list");
                self.graph.remove_edge(source_parent_edge);
                let new_weight = PathBuilder::new(siblings.len() as u32 + 1).active(true).build();
                self.graph.update_edge(target_parent, source, new_weight);
                self.normalize_edge_weights(target_parent);
                self.normalize_edge_weights(source_parent);
                Ok(target_parent)
            }
            ShiftDirection::Right => {
                trace!("place_node edge case: placing in the first place of the sibling list");
                // shift all of them over, place source node in the first place
                for sibling_ix in siblings {
                    let sibling_edge = self.graph.find_edge(target_parent, sibling_ix)
                        .expect("Sibling to target was not linked to target's parent");
                    let weight = self.graph.edge_weight_mut(sibling_edge)
                        .expect("Could not get the weight of the edge between target sibling and target parent");
                    trace!("Sibling {:?} previously had an edge weight of {:?} to {:?}", sibling_ix, weight, target_parent);
                    **weight = **weight + 1;
                    trace!("Deactivating path {:?}", sibling_edge);
                    weight.active = false;
                    trace!("Sibling {:?}, edge weight to {:?} is now {:?}", sibling_ix, target_parent, weight);
                }
                self.graph.remove_edge(source_parent_edge);
                let new_weight = PathBuilder::new(1u32).active(true).build();
                self.graph.update_edge(target_parent, source, new_weight);
                self.normalize_edge_weights(target_parent);
                self.normalize_edge_weights(source_parent);
                Ok(target_parent)
            }
        }
    }

    /// Detaches a node from the tree (causing there to be two trees).
    /// This should only be done temporarily.
    fn detach(&mut self, node_ix: NodeIndex) {
        if let Ok(parent_ix) = self.parent_of(node_ix) {
            let edge = self.graph.find_edge(parent_ix, node_ix)
                .expect("detatch: Node has parent but edge cannot be found!");

            self.graph.remove_edge(edge);
            self.normalize_edge_weights(parent_ix);
        }
        else {
            trace!("detach: Detached a floating node");
        }
    }

    /// Removes a node at the given index. This may invalidate other node
    /// indices.
    ///
    /// From `petgraph`:
    /// Remove a from the graph if it exists, and return its weight.
    ///
    /// If it doesn't exist in the graph, return None.
    ///
    /// Apart from a, this invalidates the last node index in the graph
    /// (that node will adopt the removed node index).
    /// Edge indices are invalidated as they would be following the removal
    /// of each edge with an endpoint in a.
    ///
    /// Computes in O(e') time, where e' is the number of affected edges,
    /// including n calls to .remove_edge() where n is the number of edges
    /// with an endpoint in a, and including the edges with an endpoint in
    /// the displaced node.
    pub fn remove(&mut self, node_ix: NodeIndex) -> Option<Container> {
        let id;
        let mut maybe_view = None;
        {
            let container = &self.graph[node_ix];
            id = container.get_id();
            if let Container::View { handle, .. } = *container {
                maybe_view = Some(handle);
            }
        }
        //let id = self.graph[node_ix].get_id();
        let last_ix = NodeIndex::new(self.graph.node_count() - 1);
        let maybe_parent_ix = self.parent_of(node_ix);
        if last_ix != node_ix {
            // The container at last_ix will now have node_ix as its index
            // Have to update the id map
            let last_container = &self.graph[last_ix];
            let last_id = last_container.get_id();
            self.id_map.insert(last_id, node_ix);
            if let Container::View { handle, .. } = *last_container {
                self.view_map.insert(handle, node_ix);
            }
        }
        if let Ok(mut parent_ix) = maybe_parent_ix {
            if parent_ix == last_ix {
                parent_ix = node_ix;
            }
            self.id_map.remove(&id);
            if let Some(view) = maybe_view {
                self.view_map.remove(&view);
            }
            let result = self.graph.remove_node(node_ix);
            // Fix the edge weights of the siblings of this node,
            // so we don't leave a gap
            self.normalize_edge_weights(parent_ix);
            result
        } else {
            self.id_map.remove(&id);
            self.graph.remove_node(node_ix)
        }
    }

    /// Normalizes the edge weights of the children of a node so that there are no gaps
    fn normalize_edge_weights(&mut self, parent_ix: NodeIndex) {
        let mut weight = Path::zero();
        for child_ix in self.children_of(parent_ix) {
            let edge = self.graph.find_edge(parent_ix, child_ix)
                .expect("Child was not linked to it's parent");
            let edge_weight = self.graph.edge_weight_mut(edge)
                .expect("Could not get the weight of the edge between target sibling and target parent");
            if **edge_weight != *weight + 1 {
                trace!("Normalizing edge {:?} to {:?}", edge_weight, **edge_weight.deref() + 1);
                *edge_weight.deref_mut() = *weight + 1;
            }
            weight = *edge_weight;
        }
        trace!("Normalized edge weights for: {:?}", parent_ix);
    }

    /// Determines if the container node can be removed because it is empty.
    /// If it is a non-root container then it can never be removed.
    pub fn can_remove_empty_parent(&self, container_ix: NodeIndex) -> bool {
        if self.graph[container_ix].get_type() != ContainerType::Container
        || self.is_root_container(container_ix) {
            return false
        }
        if self.children_of(container_ix).len() == 0 {
            true
        } else {
            false
        }
    }

    /// Moves a node between two indices
    pub fn move_node(&mut self, node_ix: NodeIndex, new_parent: NodeIndex) {
        self.detach(node_ix);
        self.attach_child(new_parent, node_ix);
    }

    /// Whether a node has a parent
    pub fn has_parent(&self, node_ix: NodeIndex) -> bool {
        let neighbors = self.graph
            .neighbors_directed(node_ix, EdgeDirection::Incoming);
        match neighbors.count() {
            0 => false,
            1 => true,
            _ => panic!("Node has more than one parent!")
        }
    }

    /// Gets the parent of a node, if the node exists
    pub fn parent_of(&self, node_ix: NodeIndex) -> Result<NodeIndex, GraphError> {
        let mut neighbors = self.graph
            .neighbors_directed(node_ix, EdgeDirection::Incoming);
        let result = neighbors.next().ok_or(GraphError::NoParent(node_ix));
        if cfg!(debug_assertions) {
            if neighbors.next().is_some() {
                error!("{:?}", self);
                panic!("parent_of: node has multiple parents!")
            }
	}
        result
    }

    /// Collects all children of a node, sorted by weight.
    ///
    /// Will return an empty iterator if the node has no children or
    pub fn children_of(&self, node_ix: NodeIndex) -> Vec<NodeIndex> {
        let mut edges = self.graph.edges_directed(node_ix, EdgeDirection::Outgoing)
            .collect::<Vec<(NodeIndex, &Path)>>();
        edges.sort_by_key(|&(ref _ix, ref edge)| *edge);
        edges.into_iter().map(|(ix, _edge)| ix).collect()
    }

    /// Collects all **floating** children of a node, sorted by weight
    ///
    /// Will return an empty iterator if the node has no children or
    pub fn floating_children(&self, node_ix: NodeIndex) -> Vec<NodeIndex> {
        let mut edges = self.graph.edges_directed(node_ix, EdgeDirection::Outgoing)
            .filter(|&(node_ix, _)| self[node_ix].floating())
            .collect::<Vec<(NodeIndex, &Path)>>();
        edges.sort_by_key(|&(ref _ix, ref edge)| *edge);
        edges.into_iter().map(|(ix, _edge)| ix).collect()
    }

    /// Collects all **non-floating** children of a node, sorted by weight
    ///
    /// Will return an empty iterator if the node has no children or
    pub fn grounded_children(&self, node_ix: NodeIndex) -> Vec<NodeIndex> {
        let mut edges = self.graph.edges_directed(node_ix, EdgeDirection::Outgoing)
            .filter(|&(node_ix, _)| !self[node_ix].floating())
            .collect::<Vec<(NodeIndex, &Path)>>();
        edges.sort_by_key(|&(ref _ix, ref edge)| *edge);
        edges.into_iter().map(|(ix, _edge)| ix).collect()
    }

    /// Looks up a container by id
    pub fn lookup_id(&self, id: Uuid) -> Option<NodeIndex> {
        self.id_map.get(&id).cloned()
    }

    pub fn lookup_view(&self, view: WlcView) -> Option<NodeIndex> {
        self.view_map.get(&view).cloned()
    }

    /// Gets the container of the given node.
    pub fn get(&self, node_ix: NodeIndex) -> Option<&Container> {
        self.graph.node_weight(node_ix)
    }

    /// Gets a mutable reference to a given node
    pub fn get_mut(&mut self, node_ix: NodeIndex) -> Option<&mut Container> {
        self.graph.node_weight_mut(node_ix)
    }

    /// Gets the ContainerType of the selected node
    pub fn node_type(&self, node_ix: NodeIndex) -> Option<ContainerType> {
        self.graph.node_weight(node_ix).map(Container::get_type)
    }

    /// Gets the index of the workspace of this name
    pub fn workspace_ix_by_name(&self, name: &str) -> Option<NodeIndex> {
        for output in self.children_of(self.root_ix()) {
            for workspace in self.children_of(output) {
                if self.graph[workspace].get_name()
                    .expect("workspace_by_name: bad tree structure") == name {
                        return Some(workspace)
                    }
            }
        }
        return None
    }

    /// Attempts to get an ancestor matching the matching type
    ///
    /// Note this does *NOT* check the given node.
    pub fn ancestor_of_type(&self, node_ix: NodeIndex,
                           container_type: ContainerType) -> Result<NodeIndex, GraphError> {
        let mut curr_ix = node_ix;
        while let Ok(parent_ix) = self.parent_of(curr_ix) {
            let parent = self.graph.node_weight(parent_ix)
                .expect("ancestor_of_type: parent_of invalid");
            if parent.get_type() == container_type {
                return Ok(parent_ix)
            }
            curr_ix = parent_ix;
        }
        return Err(GraphError::NotFound(container_type, node_ix));
    }

    /// Attempts to get a descendant of the matching type
    /// Looks down the left side of the tree first
    ///
    /// Note this *DOES* check the given node.
    pub fn descendant_of_type(&self, node_ix: NodeIndex,
                              container_type: ContainerType) -> Result<NodeIndex, GraphError> {
        if let Some(container) = self.get(node_ix) {
            if container.get_type() == container_type {
                return Ok(node_ix)
            }
        }
        for child in self.children_of(node_ix) {
            if let Ok(desc) = self.descendant_of_type(child, container_type) {
                return Ok(desc)
            }
        }
        return Err(GraphError::NotFound(container_type, node_ix))
    }

    /// Attempts to get a descendant of the matching type.
    /// Looks down the right side of the tree first
    pub fn descendant_of_type_right(&self, node_ix: NodeIndex,
                                    container_type: ContainerType) -> Result<NodeIndex, GraphError> {
        if let Some(container) = self.get(node_ix) {
            if container.get_type() == container_type {
                return Ok(node_ix)
            }
        }
        for child in self.children_of(node_ix).iter().rev() {
            if let Ok(desc) = self.descendant_of_type(*child, container_type) {
                return Ok(desc)
            }
        }
        return Err(GraphError::NotFound(container_type, node_ix))
    }

    /// Finds a node by the view handle.
    pub fn descendant_with_handle(&self, node_ix: NodeIndex, search_handle: &WlcView)
                               -> Option<NodeIndex> {
        self.get(node_ix).and_then(|node| match node {
            &Container::View { ref handle, .. } => {
                if handle == search_handle {
                    return Some(node_ix)
                }
                else {
                    return None
                }
            },
            _ => {
                for child in self.children_of(node_ix) {
                    if let Some(found) = self.descendant_with_handle(child,
                                                              search_handle) {
                        return Some(found)
                    }
                }
                return None
            }
        })
    }

    /// Returns the node indices of any node that is a descendant of a node
    pub fn all_descendants_of(&self, node_ix: NodeIndex) -> Vec<NodeIndex> {
        let mut index: usize = 0;
        let mut nodes: Vec<NodeIndex> = self.graph.edges_directed(node_ix,
                                                      EdgeDirection::Outgoing)
            .map(|(ix, _)| ix).collect();
        while index != nodes.len() {
            let cur_node: &NodeIndex = &nodes[index].clone();
            let children = self.graph.edges_directed(*cur_node,
                                                     EdgeDirection::Outgoing);
            let size_hint = children.size_hint();
            nodes.reserve(size_hint.1.unwrap_or(size_hint.0));
            for (ix, _) in children {
                nodes.push(ix);
            }
            index += 1;
        }
        nodes
    }

    /// Sets the node and its children's visibility
    pub fn set_family_visible(&mut self, node_ix: NodeIndex, visible: bool) {
        trace!("Setting {:?} to {}", node_ix, if visible {"visible"} else {"invisible"});
        self.get_mut(node_ix).map(|c| c.set_visibility(visible));
        for child in self.children_of(node_ix) {
            self.set_family_visible(child, visible);
        }
    }

    /// Determines if a Node index is the last one in the adjacency list
    /// (and so will be moved in a removal)
    pub fn is_last_ix(&self, node_ix: NodeIndex) -> bool {
        if self.graph.node_count() == 0 {
            false
        } else {
            self.graph.node_count() - 1 == node_ix.index()
        }
    }

    /// Modifies the ancestor paths so that the only complete path from the root
    /// goes to this node.
    ///
    /// If a divergent path is detected, that edge is deactivated in favor of
    /// the one that leads to this node.
    pub fn set_ancestor_paths_active(&mut self, mut node_ix: NodeIndex) {
        if cfg!(debug_assertions) {
            if self[node_ix].floating() {
                error!("{:?} was set active, but it is floating!\n{:#?}", node_ix, self);
                panic!("A node that was floating was set to be active!")
            }
        }
        // Make sure that any children of this node are inactive
        for child_ix in self.children_of(node_ix) {
            let edge_ix = self.graph.find_edge(node_ix, child_ix)
                .expect("Could not get edge index between parent and child");
            let edge = self.graph.edge_weight_mut(edge_ix)
                .expect("Could not associate edge index with an edge weight");
            edge.active = false;
        }
        while let Ok(parent_ix) = self.parent_of(node_ix) {
            for child_ix in self.children_of(parent_ix) {
                let edge_ix = self.graph.find_edge(parent_ix, child_ix)
                    .expect("Could not get edge index between parent and child");
                let edge = self.graph.edge_weight_mut(edge_ix)
                    .expect("Could not associate edge index with an edge weight");
                edge.active = false;
            }
            let edge_ix = self.graph.find_edge(parent_ix, node_ix)
                .expect("Could not get edge index between parent and child");
            let edge = self.graph.edge_weight_mut(edge_ix)
                .expect("Could not associate edge index with an edge weight");
            edge.active = true;
            node_ix = parent_ix;
        }
    }

    /// Gets the next sibling to focus on, assuming child_ix would be removed from parent_ix.
    /// If there are no other siblings to focus on, parent_ix is returned.
    ///
    /// # Panics
    /// This will panic if `child_ix` is not a child of `parent_ix`
    pub fn next_sibling(&self, node_ix: NodeIndex) -> Option<NodeIndex> {
        let parent_ix = self.parent_of(node_ix)
            .expect("Could not get parent of node!");
        let children = self.children_of(parent_ix);
        let mut prev_index = None;
        for (index, sibling_ix) in children.iter().enumerate() {
            if node_ix == *sibling_ix {
                prev_index = Some(index);
                break;
            }
        }
        if prev_index.is_none() {
            panic!("Could not find child in parent node");
        }
        let prev_index = prev_index.unwrap();
        if children.len() == 1 {
            return None
        }
        if prev_index == children.len() - 1 {
            Some(children[children.len() - 2])
        } else {
            Some(children[prev_index + 1])
        }
    }
}

use std::ops::{Index, IndexMut};

impl Index<NodeIndex> for InnerTree {
    type Output = Container;
    #[inline]
    fn index(&self, index: NodeIndex) -> &Self::Output {
        self.get(index).expect("graph_tree: node not found")
    }
}

impl IndexMut<NodeIndex> for InnerTree {
    #[inline]
    fn index_mut(&mut self, index: NodeIndex) -> &mut Self::Output {
        self.get_mut(index).expect("graph_tree: node not found")
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use super::super::container::*;
    use rustwlc::*;

    /// Makes a very basic tree.
    /// There is only one output,
    /// Two workspaces,
    /// First workspace has a single view in the root container,
    /// second workspace has a container with two views in it
    /// (the container is a child of the root container).
    ///
    /// The active container is the only view in the first workspace
    #[allow(unused_variables)]
    fn basic_tree() -> InnerTree {
        let mut tree = InnerTree::new();
        let fake_view_1 = WlcView::root();
        let fake_output = fake_view_1.clone().as_output();
        let root_ix = tree.root_ix();
        let fake_size = Size { h: 800, w: 600 };
        let fake_geometry = Geometry {
            size: fake_size.clone(),
            origin: Point { x: 0, y: 0 }
        };

        let output_ix = tree.add_child(root_ix, Container::new_output(fake_output), false);
        let workspace_1_ix = tree.add_child(output_ix,
                                                Container::new_workspace("1".to_string(),
                                                                   fake_size.clone()), false);
        let root_container_1_ix = tree.add_child(workspace_1_ix,
                                                Container::new_container(fake_geometry.clone()), false);
        let workspace_2_ix = tree.add_child(output_ix,
                                                Container::new_workspace("2".to_string(),
                                                                     fake_size.clone()), false);
        let root_container_2_ix = tree.add_child(workspace_2_ix,
                                                Container::new_container(fake_geometry.clone()), false);
        /* Workspace 1 containers */
        let wkspc_1_view = tree.add_child(root_container_1_ix,
                                                Container::new_view(fake_view_1.clone()), false);
        /* Workspace 2 containers */
        let wkspc_2_container = tree.add_child(root_container_2_ix,
                                                Container::new_container(fake_geometry.clone()), false);
        let wkspc_2_sub_view_1 = tree.add_child(wkspc_2_container,
                                                Container::new_view(fake_view_1.clone()), false);
        let wkspc_2_sub_view_2 = tree.add_child(wkspc_2_container,
                                                Container::new_view(fake_view_1.clone()), false);
        tree
    }

    #[test]
    fn test_descendents_of() {
        let basic_tree = basic_tree();
        let children_of_root = basic_tree.all_descendants_of(basic_tree.root);
        assert_eq!(children_of_root.len(), 9);
        let simple_view = basic_tree.descendant_of_type(basic_tree.root,
                                                        ContainerType::View)
            .expect("No view in the basic test tree");
        let children_of_view = basic_tree.all_descendants_of(simple_view);
        assert_eq!(children_of_view.len(), 0);
    }

    #[test]
    fn test_id() {
        let mut tree = basic_tree();
        let root_ix = tree.root_ix();
        // This is the uuid of the view, we will invalidate it in the next block
        let view_id;
        {
            let view_ix = tree.descendant_of_type(root_ix, ContainerType::View)
                .expect("Had no descendant of type ContainerType View");
            let view_container = &tree[view_ix];
            view_id = view_container.get_id();
            assert_eq!(tree.id_map.get(&view_id), Some(&view_ix));
        }
        {
            let view_ix = *tree.id_map.get(&view_id)
                .expect("View with a uuid did not exist");
            tree.remove(view_ix);
            assert_eq!(tree.id_map.get(&view_id), None);
        }
        let fake_view = WlcView::root();
        let root_container_ix = tree.descendant_of_type(root_ix, ContainerType::Container).unwrap();
        let container = Container::new_view(fake_view);
        let container_uuid = container.get_id();
        tree.add_child(root_container_ix, container.clone(), false);
        let only_view = &tree[tree.descendant_of_type(root_ix, ContainerType::View).unwrap()];
        assert_eq!(*only_view, container);
        assert_eq!(only_view.get_id(), container_uuid);

        // Generic test where we make sure all of them have the right ids in the map
        for container_ix in tree.all_descendants_of(root_ix) {
            let container = &tree[container_ix];
            let container_id = container.get_id();
            assert_eq!(*tree.id_map.get(&container_id).unwrap(), container_ix);
        }
    }

    #[test]
    // Ensures floating children_of includes floating children
    fn children_of_has_floating() {
        let mut tree = basic_tree();
        let root_ix = tree.root_ix();
        let root_c = tree.descendant_of_type(root_ix, ContainerType::Container)
            .expect("No containers in basic tree");
        let children = tree.children_of(root_c);
        assert_eq!(children.len(), 1);
        for child_ix in children {
            tree[child_ix].set_floating(true).unwrap();
        }
        let children = tree.children_of(root_c);
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn floating_children_only() {
        let mut tree = basic_tree();
        let root_ix = tree.root_ix();
        let root_c = tree.descendant_of_type(root_ix, ContainerType::Container)
            .expect("No containers in basic tree");
        let children = tree.children_of(root_c);
        assert_eq!(children.len(), 1);
        let floating_children = tree.floating_children(root_c);
        assert_eq!(floating_children.len(), 0);
        for child_ix in children {
            tree[child_ix].set_floating(true).unwrap();
        }
        let children = tree.children_of(root_c);
        assert_eq!(children.len(), 1);
        let floating_children = tree.floating_children(root_c);
        assert_eq!(floating_children.len(), 1);
        for child_ix in floating_children {
            tree[child_ix].set_floating(false).unwrap();
        }
        let children = tree.children_of(root_c);
        assert_eq!(children.len(), 1);
        let floating_children = tree.floating_children(root_c);
        assert_eq!(floating_children.len(), 0);
    }
}
