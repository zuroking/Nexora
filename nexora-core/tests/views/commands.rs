use nexora_core::commands::{all, by_shortcut, filter, CommandId};
use nexora_core::fuzzy::score;

#[test]
fn registry_has_all_commands() {
    assert_eq!(all().len(), 31);
    assert_eq!(CommandId::ALL.len(), 31);
}

#[test]
fn labels_and_shortcuts_are_unique() {
    let commands = all();
    let mut labels: Vec<_> = commands.iter().map(|c| c.label).collect();
    labels.sort_unstable();
    let before = labels.len();
    labels.dedup();
    assert_eq!(labels.len(), before);

    let shortcuts: Vec<_> = commands
        .iter()
        .filter_map(|c| c.shortcut)
        .collect();
    let mut shortcuts = shortcuts;
    shortcuts.sort_unstable();
    let before = shortcuts.len();
    shortcuts.dedup();
    assert_eq!(shortcuts.len(), before);
}

#[test]
fn shortcut_lookup_matches_spec() {
    assert_eq!(
        by_shortcut("Ctrl+N").map(|c| c.id),
        Some(CommandId::NewNote)
    );
    assert_eq!(
        by_shortcut("Ctrl+Shift+N").map(|c| c.id),
        Some(CommandId::QuickCapture)
    );
    assert_eq!(
        by_shortcut("Ctrl+F").map(|c| c.id),
        Some(CommandId::Search)
    );
    assert_eq!(
        by_shortcut("Ctrl+W").map(|c| c.id),
        Some(CommandId::CloseTab)
    );
    assert_eq!(
        by_shortcut("F2").map(|c| c.id),
        Some(CommandId::RenameNote)
    );
    assert_eq!(
        by_shortcut("Ctrl+O").map(|c| c.id),
        Some(CommandId::QuickOpen)
    );
    assert_eq!(
        by_shortcut("Ctrl+Shift+O").map(|c| c.id),
        Some(CommandId::GoToHeading)
    );
    assert_eq!(by_shortcut("Ctrl+P"), None);
    assert_eq!(by_shortcut("Ctrl+G"), None);
}

#[test]
fn palette_filter_ranks_subsequence_matches() {
    assert_eq!(filter("grph")[0].id, CommandId::OpenGraph);
    assert_eq!(filter(" hlth ")[0].id, CommandId::HealthCheck);
    assert!(filter("zzzzzz").is_empty());
    let empty = filter("  ");
    assert_eq!(empty.len(), 31);
    assert_eq!(empty.iter().map(|c| c.id).collect::<Vec<_>>(), CommandId::ALL.to_vec());
}

#[test]
fn palette_filter_matches_shortcuts() {
    let result = filter("cw");
    assert!(result.iter().any(|command| command.id == CommandId::CloseTab));
    let result = filter("ctrl+shift+n");
    assert!(result.iter().any(|command| command.id == CommandId::QuickCapture));
}

#[test]
fn palette_filter_is_deterministic() {
    // Both "Save query" and "Save workspace" must score equally; the earlier
    // registry entry then wins so Enter is predictable (§67).
    assert_eq!(
        score("save", CommandId::SaveQuery.label()),
        score("save", CommandId::SaveWorkspace.label())
    );
    assert_eq!(filter("save")[0].id, CommandId::SaveQuery);
    let first = filter("o");
    let second = filter("o");
    assert_eq!(
        first.iter().map(|c| c.id).collect::<Vec<_>>(),
        second.iter().map(|c| c.id).collect::<Vec<_>>()
    );
    let mut ids: Vec<_> = first.iter().map(|c| c.id).collect();
    ids.sort_unstable_by_key(|id| *id as usize);
    ids.dedup();
    assert_eq!(ids.len(), first.len());
}

#[test]
fn palette_filter_is_case_insensitive() {
    let result = filter("graph");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, CommandId::OpenGraph);
    assert!(filter("  HEALTH  ")[0].id == CommandId::HealthCheck);
    assert!(filter("nonexistent").is_empty());
    assert_eq!(filter("").len(), 31);
    assert_eq!(filter("theme")[0].id, CommandId::CycleTheme);
    let workspaces = filter("workspace");
    assert!(workspaces.iter().any(|c| c.id == CommandId::SaveWorkspace));
    assert!(workspaces.iter().any(|c| c.id == CommandId::ApplyWorkspace));
}

