mod actions;
mod core;
pub mod commands;

pub use self::actions::movement::MovementError;

pub use self::core::action::{Action, ActionErr};
pub use self::core::container::{Container, ContainerType, Handle, Layout};
pub use self::core::tree::{Direction, TreeError};
use self::core::InnerTree;

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
    static ref PREV_ACTION: Mutex<Option<Action>> = Mutex::new(None);
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

/// Attempts to lock the tree. If the Result is Err, then the lock could
/// not be returned at this time, already locked.
pub fn try_lock_tree() -> Result<Tree, TreeErr> {
    let tree = try!(TREE.try_lock());
    Ok(Tree(tree))
}

/// Attempts to lock the action mutex. If the Result is Err, then the lock could
/// not be returned at this time, already locked.
pub fn try_lock_action() -> Result<MutexGuard<'static, Option<Action>>,
                                 TryLockError<MutexGuard<'static,
                                                         Option<Action>>>> {
    PREV_ACTION.try_lock()
}

pub fn tree_as_json() -> Json {
    if let Ok(tree) = try_lock_tree() {
        tree.0.to_json()
    } else {
        Json::Null
    }
}
