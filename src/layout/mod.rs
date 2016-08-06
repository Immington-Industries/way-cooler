mod layout_tree;
mod graph_tree;
mod container;
pub mod commands;

pub use self::container::{Container, ContainerType, Handle, Layout};
pub use self::layout_tree::tree::{Direction};
use self::graph_tree::InnerTree;

use petgraph::graph::NodeIndex;
use rustc_serialize::json::{Json, ToJson};

use std::sync::{Mutex, MutexGuard, TryLockError};

/// A wrapper around tree, to hide its methods
pub struct Tree(TreeGuard);
/// Mutex guard around the tree
pub type TreeGuard = MutexGuard<'static, LayoutTree>;
/// Error for trying to lock the tree
pub type TreeErr = TryLockError<TreeGuard>;
/// Result for locking the tree
pub type TreeResult = Result<MutexGuard<'static, LayoutTree>, TreeErr>;


#[derive(Debug)]
pub struct LayoutTree {
    tree: InnerTree,
    active_container: Option<NodeIndex>
}

lazy_static! {
    static ref TREE: Mutex<LayoutTree> = {
        Mutex::new(LayoutTree {
            tree: InnerTree::new(),
            active_container: None
        })
    };
}

impl ToJson for LayoutTree {
    fn to_json(&self) -> Json {
        use std::collections::BTreeMap;
        fn node_to_json(node_ix: NodeIndex, tree: &LayoutTree) -> Json {
            match &tree.tree[node_ix] {
                &Container::Workspace { ref name, .. } => {
                    let mut inner_map = BTreeMap::new();
                    let children = tree.tree.children_of(node_ix).iter()
                        .map(|node| node_to_json(*node, tree)).collect();
                    inner_map.insert(format!("Workspace {}", name), Json::Array(children));
                    return Json::Object(inner_map);
                }
                &Container::Container { ref layout, .. } => {
                    let mut inner_map = BTreeMap::new();
                    let children = tree.tree.children_of(node_ix).iter()
                        .map(|node| node_to_json(*node, tree)).collect();
                    inner_map.insert(format!("Container w/ layout {:?}", layout), Json::Array(children));
                    return Json::Object(inner_map);
                }
                &Container::View { ref handle, .. } => {
                    return Json::String(handle.get_title());
                },
                ref container => {
                    let mut inner_map = BTreeMap::new();
                    let children = tree.tree.children_of(node_ix).iter()
                        .map(|node| node_to_json(*node, tree)).collect();
                    inner_map.insert(format!("{:?}", container.get_type()),
                                     Json::Array(children));
                    return Json::Object(inner_map)
                }
            }
        }
        return node_to_json(self.tree.root_ix(), self);
    }
}

/// Attempts to lock the tree. If the Result is Err, then a thread that
/// previously had the lock panicked and potentially left the tree in a bad state
pub fn try_lock_tree() -> Result<Tree, TreeErr> {
    trace!("Locking the tree!");
    let tree = try!(TREE.try_lock());
    Ok(Tree(tree))
}


pub fn tree_as_json() -> Json {
    if let Ok(tree) = try_lock_tree() {
        tree.0.to_json()
    } else {
        Json::Null
    }
}
