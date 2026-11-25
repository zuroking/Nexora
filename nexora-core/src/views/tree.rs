//! Hierarchical view of note paths for the explorer panel.
//!
//! Folders are implicit prefixes of `NoteId`; no filesystem metadata is
//! stored. The tree is rebuilt from the index, so it can never disagree with
//! the vault. See `ARCHITECTURE.md` §17.

use crate::NoteId;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Folder,
    Note(NoteId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub kind: NodeKind,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// Stable GUI identity. A folder `foo/` and a note `foo.md` are both
    /// legal and would otherwise share `path == "foo"` (§17).
    pub fn id_key(&self) -> String {
        match self.kind {
            NodeKind::Folder => format!("folder:{}", self.path),
            NodeKind::Note(_) => format!("note:{}", self.path),
        }
    }
}

#[derive(Clone, Default)]
struct Entry {
    children: BTreeMap<String, Entry>,
    note: bool,
}

fn insert(root: &mut BTreeMap<String, Entry>, id: &NoteId) {
    let parts: Vec<&str> = id.as_str().split('/').collect();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return;
    }
    let mut level = root;
    for part in &parts[..parts.len() - 1] {
        level = &mut level.entry((*part).to_string()).or_default().children;
    }
    level.entry(parts[parts.len() - 1].to_string()).or_default().note = true;
}

fn convert(
    entries: BTreeMap<String, Entry>,
    parent: &str,
) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    // Editors show folders first, then files, each alphabetically.
    for (name, entry) in entries.iter().filter(|(_, value)| !value.children.is_empty()) {
        let path = if parent.is_empty() {
            name.clone()
        } else {
            format!("{parent}/{name}")
        };
        nodes.push(TreeNode {
            name: name.clone(),
            path: path.clone(),
            kind: NodeKind::Folder,
            children: convert(entry.children.clone(), &path),
        });
    }
    for (name, _) in entries.iter().filter(|(_, value)| value.note) {
        let path = if parent.is_empty() {
            name.clone()
        } else {
            format!("{parent}/{name}")
        };
        nodes.push(TreeNode {
            name: name.clone(),
            path: path.clone(),
            kind: NodeKind::Note(NoteId::new(path)),
            children: Vec::new(),
        });
    }
    nodes
}

/// Build a sorted tree from an arbitrary set of note ids.
pub fn build<I>(ids: I) -> Vec<TreeNode>
where
    I: IntoIterator<Item = NoteId>,
{
    let mut root: BTreeMap<String, Entry> = BTreeMap::new();
    for id in ids {
        insert(&mut root, &id);
    }
    convert(root, "")
}
