//! It's a tree!

use std::ptr;

use rustwlc::handle::WlcView;

use super::container::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Node {
    // We need a mut pointer so we can modify the parent
    parent: *mut Node,
    val: Container,
    children: Vec<Node>
}

impl Node {
    /// Create a new node with the existing value.
    /// For root-style constructors.
    pub fn new(val: Container) -> Node {
        Node {
            parent: ptr::null_mut(),
            val: val,
            children: Vec::new()
        }
    }

    /// Add a new child node to this node, using a value
    pub fn new_child(&mut self, val: Container) -> &mut Node {
        let self_mut = self as *mut Node;
        self.children.push(Node {
            parent: self_mut,
            val: val,
            children: Vec::new()
        });
        let last_ix = self.children.len() -1;
        &mut self.children[last_ix]
    }

    /// Whether this node has a (currently-reachable) parent
    #[allow(dead_code)]
    pub fn has_parent(&self) -> bool {
        !self.parent.is_null()
    }

    /// Gets the type of container this node holds
    pub fn get_container_type(&self) -> ContainerType {
        self.val.get_type()
    }

    /// Tries to get an ancestor of the requested type
    pub fn get_ancestor_of_type(&self, container_type: ContainerType)
                                -> Option<&mut Node> {
        let mut maybe_parent = self.get_parent();
        loop {
            if let Some(parent) = maybe_parent {
                if parent.get_container_type() == container_type {
                    return Some(parent);
                }
                maybe_parent = parent.get_parent();
            }
            else {
                return None;
            }
        }
    }

    /// Gets a node by handle
    pub fn find_view_by_handle(&self, view_handle: &WlcView) -> Option<&Node>{
        match self.get_val() {
            &Container::View { ref handle, .. } => {
                if view_handle == handle {
                    Some(self)
                } else {
                    None
                }
            },
            _ => {
                for child in self.get_children() {
                    if let Some(view) = child.find_view_by_handle(view_handle) {
                        return Some(view);
                    }
                }
                return None;
            }
        }
    }

    /// Gets the parent of this node (if it exists)
    pub fn get_parent(&self) -> Option<&mut Node> {
        if self.parent.is_null() {
            return None;
        }
        unsafe {
            return Some(&mut *self.parent);
        }
    }

    /// Borrow the children of this node.
    pub fn get_children(&self) -> &[Node] {
        &self.children
    }

    /// Mutably borrow the children of this mutable node
    pub fn get_children_mut(&mut self) -> &mut[Node] {
        &mut self.children
    }

    /// Remove a child at the given index
    #[allow(dead_code)]
    pub fn remove_child_at(&mut self, index: usize) -> Option<Node> {
        if index > self.children.len() - 1 {
            return None;
        }
        let mut child = self.children.remove(index);
        child.parent = ptr::null_mut();
        return Some(child);
    }

    /// Moves another node to be a sibling of this node.
    #[allow(dead_code)]
    pub fn add_sibling(&self, node: Node) -> Result<(), ()> {
        if let Some(parent) = self.get_parent() {
            node.move_to(parent);
            Ok(())
        }
        else {
            Err(())
        }
    }

    /// Whether this node is a parent of another node
    #[allow(dead_code)]
    pub fn is_parent_of(&self, other: &Node) -> bool {
        // Fun fact, other.parent == self as *const Node won't compile
        self as *const Node == other.parent
    }

    /// Remove a node from its parent.
    /// This method will mutate the parent if it exists.
    pub fn remove_from_parent(&mut self) -> Option<Node> {
        let mut result: Option<Node> = None;
        if let Some(mut parent) = self.get_parent() {
            if let Some(index) = parent.children.iter().position(|c| c == self) {
                result = Some(parent.children.remove(index));
            }
        }
        self.parent = ptr::null_mut();
        result
    }

    /// Removes a child from self
    pub fn remove_child(&mut self, other: &Node) -> Option<Node> {
        if let Some(index) = self.children.iter().position(|c| c == other) {
            let mut child = self.children.remove(index);
            child.parent = ptr::null_mut();
            Some(child)
        }
        else {
            None
        }
    }

    /// Removes this node from its current parent and adds it to the given node
    /// as a child
    pub fn move_to(mut self, new_parent: &mut Node) {
        self.remove_from_parent();
        self.parent = new_parent as *mut Node;
        new_parent.children.push(self);
    }

    /// Gets the container that this node has
    pub fn get_val(&self) -> &Container {
        &self.val
    }

    /// Sets the visibility of the container and its children
    pub fn set_visibility(&mut self, visibility: bool) {
        self.val.set_visibility(visibility);
        warn!("Children: {}", self.get_children().len());
        for child in self.get_children_mut() {
            child.set_visibility(visibility);
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let children: &mut Vec<Node> = &mut self.children;
        for mut child in children {
            child.parent = ptr::null_mut();
        }
    }
}

unsafe impl Sync for Node {}
unsafe impl Send for Node {}

#[cfg(test)]
mod tests {
    use super::Node;
    use super::super::container::*;

    /// Nodes can have children added to them
    #[test]
    fn new_child() {
        let mut root = Node::new(Container::Root);
        root.new_child(Container::Root);
        root.new_child(Container::Root); // This is okay
        {
            let mut third_child = root.new_child(Container::Root);
            third_child.new_child(Container::Root);
            //root.new_child(Root); // Have to wait for 3rd child to drop
        }
        root.new_child(Container::Root); // Now this works
        assert_eq!(root.children.len(), 4);
    }

    #[test]
    fn has_get_parent() {
        let mut root = Node::new(Container::Root);
        assert!(!root.has_parent(), "Root has a parent");
        assert_eq!(root.get_parent(), None);

        let child = root.new_child(Container::Root);
        assert!(child.has_parent(), "Child does not have parent");
        assert!(child.get_parent().is_some(), "Child does not have parent");
        let parent = child.get_parent().expect("Asserted child has parent");
        assert_eq!(parent.get_container_type(), ContainerType::Root);
    }

    #[test]
    fn get_container_type() {
        let mut root = Node::new(Container::Root);
        assert_eq!(root.get_container_type(), ContainerType::Root);
        {
            let wksp = root.new_child(
                Container::Workspace { name: "Foo".to_string(),
                                       focused: false });
            assert_eq!(wksp.get_container_type(), ContainerType::Workspace);
        }
        {
            let container = root.new_child(Container::Container {
                layout: Layout::Horizontal, visible: false,
                floating: false, focused: false
            });
            assert_eq!(container.get_container_type(), ContainerType::Container);
        }
    }

    #[test]
    fn get_children() {
        // Create a root with 3 children. The 3rd child has 2 children.
        let mut root = Node::new(Container::Root);
        root.new_child(Container::Root);
        root.new_child(Container::Root);
        {
            let third_child = root.new_child(Container::Root);
            third_child.new_child(Container::Root);
            third_child.new_child(Container::Root);
        }
        let root_children = root.get_children();
        assert!(!root_children.is_empty(), "Root has children");
        assert_eq!(root_children.len(), 3);
        assert!(root_children.last().is_some(), "Root has final child");
        let third_child = root_children.last().expect("Asserted unwrap!");
        assert_eq!(third_child.get_children().len(), 2);
        let third_child_second_child = third_child.get_children().last()
            .expect("Asserted unwrap!");
        assert!(third_child_second_child.get_children().is_empty(),
                "Grandchild doesn't have children");
    }

    #[test]
    fn get_children_mut() {
        // Start out with one child, use get_mut_children to add grandchild.
        let mut root = Node::new(Container::Root);
        root.new_child(Container::Root);

        assert!(root.get_children().last().expect("Root has child")
                .get_children().is_empty(), "Root has no grandchildren");

        {
            let mut children = root.get_children_mut();
            assert_eq!(children.len(), 1);

            let mut child = &mut children[0];
            child.new_child(Container::Root);
        }

        assert_eq!(root.get_children().last().expect("Asserted unwrap!")
                .get_children().len(), 1);
    }

    #[test]
    fn remove_child_at() {
        let mut root = Node::new(Container::Root);
        root.new_child(Container::Root);
        root.new_child(Container::Workspace {
            name: "Foo".to_string(), focused: false
        });
        root.new_child(Container::Root);

        let worksp = root.remove_child_at(1).expect("Index should be valid");
        assert_eq!(worksp.get_container_type(), ContainerType::Workspace);
        assert_eq!(root.get_children().len(), 2)
    }
}
