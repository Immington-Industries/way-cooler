//! A tree represented via a petgraph graph, used for way-cooler's
//! layout.

use std::iter::Iterator;

use petgraph::EdgeDirection;
use petgraph::graph::{Graph, Node, Neighbors, NodeIndex, EdgeIndex};

use rustwlc::WlcView;

use layout::{Container, ContainerType};

/// Layout tree implemented with petgraph.
#[derive(Debug)]
pub struct Tree {
    graph: Graph<Container, ()>, // Directed graph
    root: NodeIndex
}

impl Tree {
    /// Creates a new layout tree with a root node.
    pub fn new() -> Tree {
        let mut graph = Graph::new();
        let root_ix = graph.add_node(Container::Root);
        Tree { graph: graph, root: root_ix }
    }

    /// Gets the index of the tree's root node
    pub fn root_ix(&self) -> NodeIndex {
        self.root
    }

    /// Adds a new child to a node at the index, returning the edge index
    /// of their connection and the index of the new node.
    // TODO should this return a result like the old API?
    pub fn add_child(&mut self, parent_ix: NodeIndex, val: Container)
                     -> (EdgeIndex, NodeIndex) {
        let parent = self.graph.node_weight(parent_ix)
            .expect("add_child: parent not found");
        if !parent.get_type().can_have_child(val.get_type()) {
            panic!("Attempted to give a {:?} a {:?} child!",
                   parent.get_type(), val.get_type())
        }
        let child_ix = self.graph.add_node(val);
        let edge_ix = self.graph.update_edge(parent_ix, child_ix, ());
        (edge_ix, child_ix)
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

        return self.graph.update_edge(parent_ix, child_ix, ())
    }

    /// Detaches a node from the tree (causing there to be two trees).
    /// This should only be done temporarily.
    pub fn detach(&mut self, node_ix: NodeIndex) {
        let mut result: Option<NodeIndex> = None;
        if let Some(edge) = if cfg!(debug_assertions) {
            let edges = self.graph
                .neighbors_directed(node_ix, EdgeDirection::Incoming);
            let result = edges.next();
            if edges.next().is_some() {
                panic!("detach: node had more than one parent!")
            }
        }
        else {
            self.graph.neighbors_directed(node_ix, EdgeDirection::Incoming)
                .next()
        } {
            self.graph.remove_edge(edge);
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
        unimplemented!();
        self.graph.remove_node(node_ix)
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
        match neighbors.iter().count() {
            0 => false,
            1 => true,
            _ => panic!("Node has more than one parent!")
        }
    }

    /// Gets the parent of a node, if the node exists
    pub fn parent_of(&self, node_ix: NodeIndex) -> Option<NodeIndex> {
        let neighbors = self.graph
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
    pub fn children_of(&self, node_ix: NodeIndex) -> Neighbors<NodeIndex> {
        self.graph.neighbors_directed(node_ix, EdgeDirection::Outgoing)
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
    pub fn node_type(&self, node_ix: NodeIndex) -> ContainerType {
        let node = self.graph.node_weight(node_ix)
            .expect("node_type: node not found");
        node.get_type()
    }

    /// Attempts to get an ancestor matching the matching type
    pub fn ancestor_of_type(&self, node_ix: NodeIndex,
                           container_type: ContainerType) -> Option<NodeIndex> {
        let mut curr_ix = node_ix;
        while let Some(parent_ix) = self.parent_of(curr_ix) {
            curr_ix = parent_ix;
            let parent = self.graph.node_weight(parent_ix)
                .expect("ancestor_of_type: parent_of invalid");
            if parent.get_type() == container_type() {
                return Some(parent_ix)
            }
            curr_ix = parent_ix;
        }
        return None;
    }

    /// Attempts to get a descendant of the matching type
    pub fn descendant_of_type(&self, node_ix: NodeIndex,
                           container_type: ContainerType) -> Option<NodeIndex> {
        // TODO if self == type?
        for child in self.children_of(node_ix) {
            if let Some(desc) = self.descendant_of_type(child, container_type) {
                    return Some(desc)
            }
        }
        return None
    }

    /// Finds a node by the view handle.
    pub fn descendant_with_handle(&self, node: NodeIndex, handle: &WlcView)
                               -> Option<NodeIndex> {
        match self.get(node) {
            &Container::View { ref node_handle, .. } => {
                if node_handle == handle {
                    Some(node)
                }
                else {
                    None
                }
            },
            _ => {
                for child in self.children_of(node) {
                    if let Some(view) = self.descendant_with_handle(handle) {
                        return Some(view)
                    }
                }
                return None
            }
        }
    }

    /// Sets the node and its children's visibility
    pub fn set_family_visible(&mut self, node_ix: NodeIndex, visible: bool) {
        self.get_mut(node_ix).set_visibility(visible);
        for child in self.children_of(node_ix) {
            self.get_mut(child).set_visibility(visible);
        }
    }
}

use std::ops::{Index, IndexMut};

impl Index<NodeIndex> for Tree {
    type Output = Container;
    #[inline]
    fn index(&self, index: NodeIndex) -> &Self::Output {
        self.get(index).expect("graph_tree: node not found")
    }
}

impl IndexMut<NodeIndex> for Tree {
    #[inline]
    fn index_mut(&mut self, index: NodeIndex) -> &mut Self::Output {
        self.get_mut(index).expect("graph_tree: node not found")
    }
}
