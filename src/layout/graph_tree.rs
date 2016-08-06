//! A tree represented via a petgraph graph, used for way-cooler's
//! layout.

use std::iter::Iterator;
use std::collections::HashMap;

use petgraph::EdgeDirection;
use petgraph::graph::{Graph, NodeIndex, EdgeIndex};
use uuid::Uuid;

use rustwlc::WlcView;

use layout::{Container, ContainerType};

/// Layout tree implemented with petgraph.
#[derive(Debug)]
pub struct InnerTree {
    graph: Graph<Container, u32>, // Directed graph
    id_map: HashMap<Uuid, NodeIndex>,
    root: NodeIndex
}

impl InnerTree {
    /// Creates a new layout tree with a root node.
    pub fn new() -> InnerTree {
        let mut graph = Graph::new();
        let root_ix = graph.add_node(Container::Root);
        InnerTree { graph: graph,
               id_map: HashMap::new(),
               root: root_ix
        }
    }

    /// Determines if the container at the node index is the root.
    /// Normally, this should only be true if the NodeIndex value is 1.
    pub fn is_root_container(&self, node_ix: NodeIndex) -> bool {
        if let Some(parent_ix) = self.parent_of(node_ix) {
            self.graph[parent_ix].get_type() == ContainerType::Workspace
        } else {
            false
        }
    }


    /// Gets the index of the tree's root node
    pub fn root_ix(&self) -> NodeIndex {
        self.root
    }

    /// Gets the weight of a possible edge between two notes
    pub fn get_edge_weight_between(&self, parent_ix: NodeIndex,
                                   child_ix: NodeIndex) -> Option<&u32> {
        self.graph.find_edge(parent_ix, child_ix)
            .and_then(|edge_ix| self.graph.edge_weight(edge_ix))
    }

    /// Gets the edge value of the largest child of the node
    fn largest_child(&self, node: NodeIndex) -> (NodeIndex, u32) {
        use std::cmp::{Ord, Ordering};
        self.graph.edges_directed(node, EdgeDirection::Outgoing)
            .fold((node, 0), |(old_node, old_edge), (new_node, new_edge)|
                  match <u32 as Ord>::cmp(&old_edge, new_edge) {
                      Ordering::Less => (new_node, *new_edge),
                      Ordering::Greater => (old_node, old_edge),
                      Ordering::Equal =>
                          panic!("largest_child: Node {:?} had two equal children {}",
                          node, old_edge)
                  })
    }

    /// Adds a new child to a node at the index, returning the node index
    /// of the new child node.
    pub fn add_child(&mut self, parent_ix: NodeIndex, val: Container) -> NodeIndex {
        let id = val.get_id().unwrap();
        let child_ix = self.graph.add_node(val);
        self.attach_child(parent_ix, child_ix);
        self.id_map.insert(id, child_ix);
        child_ix
    }

    /// Add an existing node (detached in the graph) to the tree.
    /// Note that floating nodes shouldn't exist for too long.
    pub fn attach_child(&mut self, parent_ix: NodeIndex, child_ix: NodeIndex)
                     -> EdgeIndex {
        // Make sure the child doesn't have a parent
        if cfg!(debug_assertions) && self.has_parent(child_ix) {
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
        let (_ix, biggest_child) = self.largest_child(parent_ix);
        self.graph.update_edge(parent_ix, child_ix, biggest_child + 1)
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
            let edge_weight = *self.get_edge_weight_between(parent_ix, sibling_ix)
                .expect("Sibling had no edge weight");
            if edge_weight < child_pos {
                continue;
            }
            self.graph.update_edge(parent_ix, sibling_ix, counter);
            counter += 1;
        }
        self.graph.update_edge(parent_ix, child_ix, child_pos);
    }

    /// Detaches a node from the tree (causing there to be two trees).
    /// This should only be done temporarily.
    fn detach(&mut self, node_ix: NodeIndex) {
        if let Some(parent_ix) = self.parent_of(node_ix) {
            let edge = self.graph.find_edge(parent_ix, node_ix)
                .expect("detatch: Node has parent but edge cannot be found!");

            self.graph.remove_edge(edge);
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
        let id = self.graph[node_ix].get_id().unwrap();
        let last_ix: NodeIndex<u32> = NodeIndex::new(self.graph.node_count() - 1);
        if last_ix != node_ix {
            // The container at last_ix will now have node_ix as its index
            // Have to update the id map
            let last_container = &self.graph[last_ix];
            let last_id = last_container.get_id().expect("Could not get container id");
            self.id_map.insert(last_id, node_ix);
        }
        self.id_map.remove(&id);
        self.graph.remove_node(node_ix)
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
    #[allow(dead_code)]
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
    pub fn parent_of(&self, node_ix: NodeIndex) -> Option<NodeIndex> {
        let mut neighbors = self.graph
            .neighbors_directed(node_ix, EdgeDirection::Incoming);
        if cfg!(debug_assertions) {
            let result = neighbors.next();
            if neighbors.next().is_some() {
                panic!("parent_of: node has multiple parents!")
            }
            result
        }
        else {
            neighbors.next()
        }
    }

    /// Gets an iterator to the children of a node.
    ///
    /// Will return an empty iterator if the node has no children or
    /// if the node does not exist.
    pub fn children_of(&self, node_ix: NodeIndex) -> Vec<NodeIndex> {
        let mut edges = self.graph.edges_directed(node_ix, EdgeDirection::Outgoing)
            .collect::<Vec<(NodeIndex, &u32)>>();
        edges.sort_by_key(|&(ref _ix, ref edge)| *edge);
        edges.into_iter().map(|(ix, _edge)| ix).collect()
    }

    /// Looks up a container by id
    pub fn lookup_id(&self, id: Uuid) -> Option<NodeIndex> {
        if let Some(node_ix) = self.id_map.get(&id) {
            Some(*node_ix)
        } else {
            None
        }
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
    pub fn ancestor_of_type(&self, node_ix: NodeIndex,
                           container_type: ContainerType) -> Option<NodeIndex> {
        let mut curr_ix = node_ix;
        while let Some(parent_ix) = self.parent_of(curr_ix) {
            let parent = self.graph.node_weight(parent_ix)
                .expect("ancestor_of_type: parent_of invalid");
            if parent.get_type() == container_type {
                return Some(parent_ix)
            }
            curr_ix = parent_ix;
        }
        return None;
    }

    /// Attempts to get a descendant of the matching type
    /// Looks down the left side of the tree first
    pub fn descendant_of_type(&self, node_ix: NodeIndex,
                           container_type: ContainerType) -> Option<NodeIndex> {
        if let Some(container) = self.get(node_ix) {
            if container.get_type() == container_type {
                return Some(node_ix)
            }
        }
        for child in self.children_of(node_ix) {
            if let Some(desc) = self.descendant_of_type(child, container_type) {
                return Some(desc)
            }
        }
        return None
    }

    /// Attempts to get a descendant of the matching type.
    /// Looks down the right side of the tree first
    pub fn descendant_of_type_right(&self, node_ix: NodeIndex,
                              container_type: ContainerType) -> Option<NodeIndex> {
        if let Some(container) = self.get(node_ix) {
            if container.get_type() == container_type {
                return Some(node_ix)
            }
        }
        for child in self.children_of(node_ix).iter().rev() {
            if let Some(desc) = self.descendant_of_type(*child, container_type) {
                return Some(desc)
            }
        }
        return None
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
    pub fn all_descendants_of(&self, node_ix: &NodeIndex) -> Vec<NodeIndex> {
        let mut index: usize = 0;
        let mut nodes: Vec<NodeIndex> = self.graph.edges_directed(*node_ix,
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

        let output_ix = tree.add_child(root_ix, Container::new_output(fake_output));
        let workspace_1_ix = tree.add_child(output_ix,
                                                Container::new_workspace("1".to_string(),
                                                                   fake_size.clone()));
        let root_container_1_ix = tree.add_child(workspace_1_ix,
                                                Container::new_container(fake_geometry.clone()));
        let workspace_2_ix = tree.add_child(output_ix,
                                                Container::new_workspace("2".to_string(),
                                                                     fake_size.clone()));
        let root_container_2_ix = tree.add_child(workspace_2_ix,
                                                Container::new_container(fake_geometry.clone()));
        /* Workspace 1 containers */
        let wkspc_1_view = tree.add_child(root_container_1_ix,
                                                Container::new_view(fake_view_1.clone()));
        /* Workspace 2 containers */
        let wkspc_2_container = tree.add_child(root_container_2_ix,
                                                Container::new_container(fake_geometry.clone()));
        let wkspc_2_sub_view_1 = tree.add_child(wkspc_2_container,
                                                Container::new_view(fake_view_1.clone()));
        let wkspc_2_sub_view_2 = tree.add_child(wkspc_2_container,
                                                Container::new_view(fake_view_1.clone()));
        tree
    }

    #[test]
    fn test_descendents_of() {
        let basic_tree = basic_tree();
        let children_of_root = basic_tree.all_descendants_of(&basic_tree.root);
        assert_eq!(children_of_root.len(), 9);
        let simple_view = basic_tree.descendant_of_type(basic_tree.root,
                                                        ContainerType::View)
            .expect("No view in the basic test tree");
        let children_of_view = basic_tree.all_descendants_of(&simple_view);
        assert_eq!(children_of_view.len(), 0);
    }

    #[test]
    fn test_id() {
        let mut tree = basic_tree();
        let root_ix = tree.root_ix();
        {
            // Root container should not have a UUID associated with it
            let root = &tree[root_ix];
            assert_eq!(root.get_id(), None);
        }
        // This is the uuid of the view, we will invalidate it in the next block
        let view_id;
        {
            let view_ix = tree.descendant_of_type(root_ix, ContainerType::View).unwrap();
            let view_container = &tree[view_ix];
            view_id = view_container.get_id().unwrap();
            assert_eq!(*tree.id_map.get(&view_id).unwrap(), view_ix);
        }
        {
            let view_ix = *tree.id_map.get(&view_id).unwrap();
            tree.remove(view_ix);
            assert_eq!(tree.id_map.get(&view_id), None);
        }
        let fake_view = WlcView::root();
        let root_container_ix = tree.descendant_of_type(root_ix, ContainerType::Container).unwrap();
        let container = Container::new_view(fake_view);
        let container_uuid = container.get_id().unwrap();
        tree.add_child(root_container_ix, container.clone());
        let only_view = &tree[tree.descendant_of_type(root_ix, ContainerType::View).unwrap()];
        assert_eq!(*only_view, container);
        assert_eq!(only_view.get_id(), Some(container_uuid));

        // Generic test where we make sure all of them have the right ids in the map
        for container_ix in tree.all_descendants_of(&root_ix) {
            let container = &tree[container_ix];
            let container_id = container.get_id().unwrap();
            assert_eq!(*tree.id_map.get(&container_id).unwrap(), container_ix);
        }
    }
}
