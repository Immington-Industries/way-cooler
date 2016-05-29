//! A tree represented via a petgraph graph, used for way-cooler's
//! layout.

use std::iter::Iterator;

use petgraph::EdgeDirection;
use petgraph::graph::{Graph, NodeIndex, EdgeIndex};

use rustwlc::WlcView;

use layout::{Container, ContainerType};

/// Layout tree implemented with petgraph.
#[derive(Debug)]
pub struct Tree {
    graph: Graph<Container, u32>, // Directed graph
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

    /// Adds a new child to a node at the index, returning the edge index
    /// of their connection and the index of the new node.
    pub fn add_child(&mut self, parent_ix: NodeIndex, val: Container) -> NodeIndex {
        let child_ix = self.graph.add_node(val);
        self.attach_child(parent_ix, child_ix);
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

    /// Detaches a node from the tree (causing there to be two trees).
    /// This should only be done temporarily.
    pub fn detach(&mut self, node_ix: NodeIndex) {
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

    /// Sets the node and its children's visibility
    pub fn set_family_visible(&mut self, node_ix: NodeIndex, visible: bool) {
        self.get_mut(node_ix).map(|c| c.set_visibility(visible));
        for child in self.children_of(node_ix) {
            self.set_family_visible(child, visible);
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
