//! Command registry shared by the palette and keyboard shortcuts.
//!
//! `CommandId` is the single source of truth: adding a command here makes it
//! available to both entry points without duplicating labels or shortcuts.

use crate::fuzzy::score;
use std::fmt;

/// All actions exposed by the v1 command palette and shortcut layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandId {
    NewNote,
    EditProperties,
    RenameNote,
    OpenToday,
    QuickCapture,
    Search,
    QuickOpen,
    GoToHeading,
    OpenGraph,
    OpenCanvas,
    OpenBase,
    ToggleBookmark,
    ManageBookmarks,
    HealthCheck,
    RestoreSnapshot,
    BuildSemanticIndex,
    CreateLinkTarget,
    CycleTheme,
    NewFromTemplate,
    InsertTemplate,
    ExtractLines,
    MergeNote,
    SaveQuery,
    ApplySavedQuery,
    SaveWorkspace,
    ApplyWorkspace,
    SaveBaseView,
    ApplyBaseView,
    ReviewQueue,
    GradeNote,
    CloseTab,
}

impl CommandId {
    /// Stable order used by the palette.
    pub const ALL: [Self; 31] = [
        Self::NewNote,
        Self::EditProperties,
        Self::RenameNote,
        Self::OpenToday,
        Self::QuickCapture,
        Self::Search,
        Self::QuickOpen,
        Self::GoToHeading,
        Self::OpenGraph,
        Self::OpenCanvas,
        Self::OpenBase,
        Self::ToggleBookmark,
        Self::ManageBookmarks,
        Self::HealthCheck,
        Self::RestoreSnapshot,
        Self::BuildSemanticIndex,
        Self::CreateLinkTarget,
        Self::CycleTheme,
        Self::NewFromTemplate,
        Self::InsertTemplate,
        Self::ExtractLines,
        Self::MergeNote,
        Self::SaveQuery,
        Self::ApplySavedQuery,
        Self::SaveWorkspace,
        Self::ApplyWorkspace,
        Self::SaveBaseView,
        Self::ApplyBaseView,
        Self::ReviewQueue,
        Self::GradeNote,
        Self::CloseTab,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::NewNote => "New note",
            Self::EditProperties => "Edit properties",
            Self::RenameNote => "Rename note",
            Self::OpenToday => "Open today's note",
            Self::QuickCapture => "Quick capture",
            Self::Search => "Search",
            Self::QuickOpen => "Quick open note",
            Self::GoToHeading => "Go to heading",
            Self::OpenGraph => "Open graph",
            Self::OpenCanvas => "Open canvas",
            Self::OpenBase => "Open base view",
            Self::ToggleBookmark => "Toggle bookmark",
            Self::ManageBookmarks => "Manage bookmarks",
            Self::HealthCheck => "Run health check",
            Self::RestoreSnapshot => "Restore snapshot",
            Self::BuildSemanticIndex => "Build vector index",
            Self::CreateLinkTarget => "Create note at broken-link target",
            Self::CycleTheme => "Cycle theme",
            Self::NewFromTemplate => "New note from template",
            Self::InsertTemplate => "Insert template into current note",
            Self::ExtractLines => "Extract lines to new note",
            Self::MergeNote => "Merge note into another",
            Self::SaveQuery => "Save current query",
            Self::ApplySavedQuery => "Apply saved query",
            Self::SaveWorkspace => "Save workspace",
            Self::ApplyWorkspace => "Apply workspace",
            Self::SaveBaseView => "Save Base view",
            Self::ApplyBaseView => "Apply saved Base view",
            Self::ReviewQueue => "Review due notes",
            Self::GradeNote => "Grade current note",
            Self::CloseTab => "Close tab",
        }
    }

    pub fn shortcut(self) -> Option<&'static str> {
        match self {
            Self::NewNote => Some("Ctrl+N"),
            Self::EditProperties => None,
            Self::RenameNote => Some("F2"),
            Self::OpenToday => None,
            Self::QuickCapture => Some("Ctrl+Shift+N"),
            Self::Search => Some("Ctrl+F"),
            Self::QuickOpen => Some("Ctrl+O"),
            Self::GoToHeading => Some("Ctrl+Shift+O"),
            Self::OpenGraph => None,
            Self::OpenCanvas => None,
            Self::OpenBase => None,
            Self::ToggleBookmark => None,
            Self::ManageBookmarks => None,
            Self::HealthCheck => None,
            Self::RestoreSnapshot => None,
            Self::BuildSemanticIndex => None,
            Self::CreateLinkTarget => None,
            Self::CycleTheme => None,
            Self::NewFromTemplate => None,
            Self::InsertTemplate => None,
            Self::ExtractLines => None,
            Self::MergeNote => None,
            Self::SaveQuery => None,
            Self::ApplySavedQuery => None,
            Self::SaveWorkspace => None,
            Self::ApplyWorkspace => None,
            Self::SaveBaseView => None,
            Self::ApplyBaseView => None,
            Self::ReviewQueue => None,
            Self::GradeNote => None,
            Self::CloseTab => Some("Ctrl+W"),
        }
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// One palette row / shortcut binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub id: CommandId,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
}

/// Registry snapshot. The palette renders this; the shortcut handler matches
/// against `shortcut`.
pub fn all() -> Vec<Command> {
    CommandId::ALL
        .iter()
        .map(|&id| Command {
            id,
            label: id.label(),
            shortcut: id.shortcut(),
        })
        .collect()
}

/// Find a command by its shortcut string, e.g. `"Ctrl+P"` is reserved for the
/// palette itself and deliberately not a command.
pub fn by_shortcut(shortcut: &str) -> Option<Command> {
    all()
        .into_iter()
        .find(|command| command.shortcut == Some(shortcut))
}

/// Rank palette rows by the shared fuzzy score over label and shortcut (§67).
/// Registry order breaks ties so Enter always targets the same first row.
pub fn filter(query: &str) -> Vec<Command> {
    let needle = query.trim();
    if needle.is_empty() {
        return all();
    }
    let mut ranked: Vec<(u32, usize, Command)> = all()
        .into_iter()
        .enumerate()
        .filter_map(|(index, command)| {
            let label = score(needle, command.label);
            let shortcut = command.shortcut.and_then(|value| score(needle, value));
            label.max(shortcut).map(|value| (value, index, command))
        })
        .collect();
    ranked.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    ranked.into_iter().map(|(_, _, command)| command).collect()
}
