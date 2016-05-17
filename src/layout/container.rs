//! Container types

use rustwlc::handle::{WlcView, WlcOutput};

/// A handle to either a view or output
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Handle {
    View(WlcView),
    Output(WlcOutput)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerType {
    /// Root container, only one exists
    Root,
    /// WlcOutput/Monitor
    Output,
    /// A workspace
    Workspace,
    /// A Container, houses views and other containers
    Container,
    /// A view (window)
    View
}

impl ContainerType {
    /// Whether this container can be used as the parent of another
    pub fn can_have_child(self, other: ContainerType) -> bool {
        use self::ContainerType::*;
        match self {
            Root => other == Output,
            Output => other == Workspace,
            Workspace => other == Container,
            Container => other == Container || other == View,
            View => false
        }
    }

    /// Whether this container can have a parent of type other
    #[allow(dead_code)]
    pub fn can_have_parent(self, other: ContainerType) -> bool {
        other.can_have_child(self)
    }
}

/// Layout mode for a container
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Layout {
    Horizontal,
    Vertical,
    Stacked,
    Tabbed,
    Floating
}

/// Represents an item in the container tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Container {
    /// Root node of the container
    Root,
    /// Output
    Output {
        /// Handle to the wlc
        handle: WlcOutput,
        /// Whether the output is focused
        focused: bool
    },
    /// Workspace
    Workspace {
        /// Name of the workspace
        name: String,
        /// Whether the workspace is focused
        ///
        /// Multiple workspaces can be focused
        focused: bool
    },
    /// Container
    Container {
        /// How the container is layed out
        layout: Layout,
        /// Whether the container is visible
        visible: bool,
        /// If the container is focused
        focused: bool,
        /// If the container is floating
        floating: bool,
    },
    /// View or window
    View {
        /// The wlc handle to the view
        handle: WlcView,
        /// Whether this view is visible
        visible: bool,
        /// Whether this view is focused
        focused: bool,
        /// Whether this view is floating
        floating: bool,
    }
}

impl Container {
    /// Creates a new root container
    pub fn new_root() -> Container {
        Container::Root
    }

    /// Creates a new output container with the given output
    pub fn new_output(handle: WlcOutput) -> Container {
        Container::Output {
            handle: handle,
            focused: false
        }
    }

    /// Creates a new workspace container with the given name
    pub fn new_workspace(name: String) -> Container {
        Container::Workspace {
            name: name, focused: false
        }
    }

    /// Creates a new container
    pub fn new_container() -> Container {
        Container::Container {
            layout: Layout::Floating, // default layout?
            visible: false,
            focused: false,
            floating: false
        }
    }

    /// Creates a new view container with the given handle
    pub fn new_view(handle: WlcView) -> Container {
        Container::View {
            handle: handle,
            visible: false,
            focused: false,
            floating: false
        }
    }

    /// Sets the visibility of this container
    pub fn set_visibility(&mut self, visibility: bool) {
        match *self {
            Container::View { ref mut visible, .. } => *visible = visibility,
            Container::Container { ref mut visible, .. } => *visible = visibility,
            _ => {return},
        }
        let mask = if visibility { 1 } else { 0 };
        if let Some(handle) = self.get_handle() {
            match handle {
                Handle::View(view) => {
                    warn!("Setting {:?} visibility to {}", view, visibility);
                    view.set_mask(mask)
                },
                _ => {},
            }
        }
    }

    /// Gets the visibility flag of this container
    #[allow(dead_code)]
    pub fn get_visibility(&mut self) -> Option<bool> {
        match *self {
            Container::View { visible, .. } => Some(visible),
            Container::Container { visible, .. } => Some(visible),
            _ => None
        }
    }

    /// Gets the type of this container
    pub fn get_type(&self) -> ContainerType {
        match *self {
            Container::Root => ContainerType::Root,
            Container::Output { .. } => ContainerType::Output,
            Container::Workspace { .. } => ContainerType::Workspace,
            Container::Container { .. } => ContainerType::Container,
            Container::View { .. } => ContainerType::View
        }
    }

    /// Gets the view handle of the view container, if this is a view container
    pub fn get_handle(&self) -> Option<Handle> {
        match *self {
            Container::View { ref handle, ..} => Some(Handle::View(handle.clone())),
            Container::Output { ref handle, .. } => Some(Handle::Output(handle.clone())),
            _ => None
        }
    }

    /// Gets the name of the workspace, if this container is a workspace.
    pub fn get_name(&self) -> Option<&str> {
        match *self {
            Container::Workspace { ref name, ..} => Some(name),
            _ => None
        }
    }

    /// Determines if the container is focused or not
    #[allow(dead_code)]
    pub fn is_focused(&self) -> bool {
        match *self {
            Container::Output { ref focused, .. } => focused.clone(),
            Container::Workspace { ref focused, .. } => focused.clone(),
            Container::View { ref focused, .. } => focused.clone(),
            _ => false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_have_child() {
        let root = ContainerType::Root;
        let output = ContainerType::Output;
        let workspace = ContainerType::Workspace;
        let container = ContainerType::Container;
        let view = ContainerType::View;

        assert!(root.can_have_child(output),         "Root      > output");
        assert!(output.can_have_child(workspace),    "Output    > workspace");
        assert!(workspace.can_have_child(container), "Workspace > container");
        assert!(container.can_have_child(container), "Container > container");
        assert!(container.can_have_child(view),      "Container > view");

        assert!(!root.can_have_child(root),      "! Root > root");
        assert!(!root.can_have_child(workspace), "! Root > workspace");
        assert!(!root.can_have_child(container), "! Root > container");
        assert!(!root.can_have_child(view),      "! Root > view");

        assert!(!output.can_have_child(root),      "! Output > root");
        assert!(!output.can_have_child(output),    "! Output > output");
        assert!(!output.can_have_child(container), "! Output > container");
        assert!(!output.can_have_child(view),      "! Output > view");

        assert!(!workspace.can_have_child(root),      "! Workspace > root");
        assert!(!workspace.can_have_child(output),    "! Workspace > output");
        assert!(!workspace.can_have_child(workspace), "! Workspace > worksp");
        assert!(!workspace.can_have_child(view),      "! Workspace > view");

        assert!(!container.can_have_child(root),      "! Container > root");
        assert!(!container.can_have_child(workspace), "! Container > worksp");
        assert!(!container.can_have_child(output),    "! Container > contanr");

        assert!(!view.can_have_child(root),      "! View > root");
        assert!(!view.can_have_child(output),    "! View > output");
        assert!(!view.can_have_child(workspace), "! View > workspace");
        assert!(!view.can_have_child(container), "! View > container");
        assert!(!view.can_have_child(view),      "! View > view");
    }
}
