//! Nexora desktop shell: egui panels, tabs, autosave wiring, command palette.
//!
//! All persistence and derived indexes live in `nexora-core`; this crate only
//! draws state and turns user input into core calls.

use eframe::egui::{self, Align2, Color32, FontId, Key, Margin, PointerButton, RichText, Sense, Stroke, TextStyle, Vec2};
use eframe::epaint::text::{FontData, FontFamily, FontInsert, FontPriority, InsertFontFamily};
use eframe::epaint::Vertex;
use egui_commonmark::{Alert, AlertBundle, CommonMarkCache, CommonMarkViewer};
use nexora_core::autocomplete::partial_at;
use nexora_core::bases::{self, BaseConfig, SortBy};
use nexora_core::bookmarks::{self, Bookmarks};
use nexora_core::canvas::{self, Canvas};
use nexora_core::commands::{self, CommandId};
use nexora_core::composer;
use nexora_core::saved_queries::{self, SavedQuery};
use nexora_core::review::{self, AGAIN, EASY, GOOD, HARD};
use nexora_core::workspace::{self, Workspace};
use nexora_core::diff::{diff, DiffKind, DiffRow};
use nexora_core::frontmatter::{read_field, read_list, write_field, write_list};
use nexora_core::fuzzy::score as fuzzy_score;
use nexora_core::outline::{self, extract as extract_headings};
use nexora_core::outgoing;
use nexora_core::preview::{self, Preview};
use nexora_core::semantic::{
    self as semantic, hash_embed, search_semantic, RankSource, SemanticIndex, DEFAULT_DIMS,
};
use nexora_core::status::saved_label;
use nexora_core::templates::{self, Template};
use nexora_core::theme::{self, Palette, ThemePreset};
use nexora_core::related::{self, Related};
use nexora_core::rename::{self, RenamePlan};
use nexora_core::display::{adapt_for_display, decode_target, encode_component};
use nexora_core::images;
use nexora_core::health::Severity;
use nexora_core::inbox::INBOX_ID;
use nexora_core::lists;
use nexora_core::tables;
use nexora_core::tabs as tab_nav;
use nexora_core::quotes;
use nexora_core::mentions;
use nexora_core::named_bases::{self, NamedBase};
use nexora_core::tree::{build as build_tree, NodeKind, TreeNode};
use nexora_core::watch::{Change, DiskMonitor};
use nexora_core::{
    daily, graph, health, inbox, save, search, vault, NoteId, NexoraError, VaultIndex,
};
use std::cell::Cell;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, UNIX_EPOCH};

const AUTOSAVE_IDLE: Duration = Duration::from_millis(700);
/// §PROD-P4: how long a GM-4 conflict suppresses autosave retries.
const CONFLICT_RETRY_COOLDOWN: Duration = Duration::from_secs(2);

// All colours come from the active `Palette` (ARCHITECTURE.md §35). The
// thread-local is set once at startup and again on every theme cycle; eframe
// runs the whole native UI on one thread, so there is no cross-thread race.
thread_local! {
    static PALETTE: Cell<Palette> = const { Cell::new(ThemePreset::Dark.palette()) };
}

fn palette() -> Palette {
    PALETTE.with(|cell| cell.get())
}

fn set_palette(preset: ThemePreset) {
    PALETTE.with(|cell| cell.set(preset.palette()));
}

fn c32(rgb: theme::Rgb) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

fn panel() -> Color32 {
    c32(palette().panel)
}
fn surface() -> Color32 {
    c32(palette().surface)
}
fn raised() -> Color32 {
    c32(palette().raised)
}
fn line() -> Color32 {
    c32(palette().line)
}
fn text() -> Color32 {
    c32(palette().text)
}
fn muted() -> Color32 {
    c32(palette().muted)
}
fn accent() -> Color32 {
    c32(palette().accent)
}
fn accent_dim() -> Color32 {
    c32(palette().accent_dim)
}
fn sage() -> Color32 {
    c32(palette().sage)
}
fn amber() -> Color32 {
    c32(palette().amber)
}
fn rust() -> Color32 {
    c32(palette().rust)
}
fn bg() -> Color32 {
    c32(palette().bg)
}
fn tag() -> Color32 {
    c32(palette().tag)
}
fn tag_hover() -> Color32 {
    c32(palette().tag_hover)
}
fn info_fg() -> Color32 {
    c32(palette().info_fg)
}
fn link() -> Color32 {
    c32(palette().link)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewMode {
    Source,
    Reading,
}

/// Which action the shared template picker is serving (§75).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TemplatePickerMode {
    NewNote,
    Insert,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SearchMode {
    Keyword,
    Semantic,
    Hybrid,
    Query,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComposerMode {
    Extract,
    Merge,
}

enum ComposerAction {
    Extract,
    Merge(NoteId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabKind {
    Note,
    Graph,
    Canvas,
    Base,
    Health,
    Search,
}

impl TabKind {
    /// One table for the persisted spelling, so label and parse cannot drift.
    const KINDS: [(Self, &'static str); 6] = [
        (Self::Note, "note"),
        (Self::Graph, "graph"),
        (Self::Canvas, "canvas"),
        (Self::Base, "base"),
        (Self::Health, "health"),
        (Self::Search, "search"),
    ];

    fn label(self) -> &'static str {
        Self::KINDS
            .iter()
            .find(|(kind, _)| *kind == self)
            .map(|(_, label)| *label)
            .unwrap_or("note")
    }

    fn parse(value: &str) -> Option<Self> {
        Self::KINDS
            .iter()
            .find(|(_, label)| *label == value)
            .map(|(kind, _)| *kind)
    }
}

struct Tab {
    id: String,
    title: String,
    kind: TabKind,
    view: ViewMode,
    content: String,
    /// What the editor saw the last time this tab was clean.
    loaded: String,
    dirty: bool,
    last_edit: Option<Instant>,
}

struct LinkPopup {
    start: usize,
    cursor: usize,
    partial: String,
    suggestions: Vec<NoteId>,
    selected: usize,
    pos: egui::Pos2,
}

struct SnapshotModal {
    id: NoteId,
    stamp: String,
    rows: Vec<DiffRow>,
    dirty: bool,
}

/// One selected Base cell. The value lives here so a rejected edit can be
/// corrected without retyping (§56).
struct BaseEdit {
    id: NoteId,
    /// Group value distinguishes rows produced by a grouped list field.
    group: String,
    /// Occurrence among rows with the same `(id, group)`; list fields can
    /// legitimately repeat a value, so group alone is not a unique row key.
    occurrence: usize,
    /// Column position: a group column may share the frontmatter key with a
    /// normal column and still needs its own widget identity.
    column: String,
    key: String,
    value: String,
}

/// One editable cell of the Base grid projection (§80).
#[derive(Clone, Debug, PartialEq, Eq)]
struct BaseCellRef {
    id: NoteId,
    group: String,
    occurrence: usize,
    column: String,
    key: String,
    value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BookmarkAction {
    Open,
    Up,
    Down,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BaseCellAction {
    None,
    Commit,
    Cancel,
}

struct Keyboard {
    quick_open: bool,
    goto_heading: bool,
    rename: bool,
    toggle_view: bool,
    palette: bool,
    new_note: bool,
    capture: bool,
    search: bool,
    close_tab: bool,
    next_tab: bool,
    prev_tab: bool,
    escape: bool,
}

struct NexoraApp {
    root: PathBuf,
    index: VaultIndex,
    /// §86: the startup index build failed; retry once per poll until it
    /// succeeds, but never re reload a legitimately empty vault.
    index_unavailable: bool,
    /// §86: one-shot notice, applied at frame end so write paths cannot
    /// overwrite the recovery status.
    recovery_notice: bool,
    tabs: Vec<Tab>,
    active: usize,
    search_query: String,
    tag_filter: Option<String>,
    collapsed: BTreeSet<String>,
    palette_open: bool,
    palette_query: String,
    palette_selected: usize,
    /// Last query that produced `palette_selected`; typing resets the row (§67).
    palette_selection_key: String,
    capture_open: bool,
    capture_text: String,
    rename_input_open: bool,
    rename_confirm_open: bool,
    rename_new: String,
    rename_plan: Option<RenamePlan>,
    snapshot_modal: Option<SnapshotModal>,
    pending_line: Option<(NoteId, usize)>,
    quick_open: bool,
    quick_query: String,
    quick_results: Vec<NoteId>,
    quick_selected: usize,
    outline_open: bool,
    outline_query: String,
    outline_results: Vec<(NoteId, usize, String)>,
    outline_selected: usize,
    properties_open: bool,
    /// Note the open Properties window was opened for (§GUI-3).
    properties_note: Option<String>,
    prop_created: String,
    prop_modified: String,
    prop_aliases: String,
    prop_tags: String,
    link_popup: Option<LinkPopup>,
    /// Dismissed `[[` anchor, so Escape survives more than one frame (§50).
    link_dismissed: Option<(usize, String)>,
    selected_broken: Option<NoteId>,
    markdown_cache: CommonMarkCache,
    semantic_index: Option<SemanticIndex>,
    related_cache: Option<(NoteId, u64, Vec<Related>)>,
    semantic_path: PathBuf,
    semantic_stale: bool,
    canvas_path: PathBuf,
    canvas: Canvas,
    base_path: PathBuf,
    base_config: BaseConfig,
    base_column_input: String,
    base_sort_input: String,
    base_edit: Option<BaseEdit>,
    /// Keyboard selection in the Base grid and its projection identity (§80).
    base_selected: Option<(usize, usize)>,
    base_selection_key: String,
    /// Exact widget id of the cell button that last took focus (§80 review).
    bookmarks_path: PathBuf,
    bookmarks: Bookmarks,
    /// Bookmark manager window (§71).
    bookmarks_open: bool,
    theme_path: PathBuf,
    theme: ThemePreset,
    theme_dirty: bool,
    templates: Vec<Template>,
    template_open: bool,
    template_picker_mode: TemplatePickerMode,
    /// Note pinned when the Insert-template picker was opened (§GUI-3).
    template_insert_note: Option<String>,
    /// Template editor: old name when editing, draft name/body (§70).
    template_editor_open: bool,
    template_edit: Option<String>,
    template_name: String,
    template_body: String,
    template_delete: Option<String>,
    composer_mode: Option<ComposerMode>,
    extract_first: String,
    extract_last: String,
    extract_new: String,
    merge_targets: Vec<NoteId>,
    /// Keyboard selection in the query manager and its list identity (§82).
    saved_query_selected: usize,
    saved_query_selection_key: String,
    saved_queries: Vec<SavedQuery>,
    query_name: String,
    /// Inline rename editor and delete confirmation in the query manager (§65).
    saved_query_rename: Option<(String, String)>,
    saved_query_delete: Option<String>,
    save_query_open: bool,
    apply_query_open: bool,
    workspaces: Vec<Workspace>,
    workspace_name: String,
    /// Inline rename editor and delete confirmation in the workspace manager (§66).
    workspace_rename: Option<(String, String)>,
    workspace_delete: Option<String>,
    save_workspace_open: bool,
    apply_workspace_open: bool,
    /// Named Base views and their manager windows (§74).
    named_bases: Vec<NamedBase>,
    base_view_name: String,
    base_view_rename: Option<(String, String)>,
    base_view_delete: Option<String>,
    save_base_view_open: bool,
    apply_base_view_open: bool,
    review_open: bool,
    grade_open: bool,
    /// Note pinned when the Grade window was opened (§GUI-3).
    grade_note: Option<String>,
    review_rows: Vec<NoteId>,
    /// Keyboard selection in the Review due notes window (§69).
    review_selected: usize,
    base_cards: bool,
    canvas_drag: Option<(NoteId, egui::Vec2)>,
    /// Keyboard selection in the Canvas tab and the node set it belongs to (§76).
    canvas_selected: Option<NoteId>,
    canvas_selection_key: String,
    /// Keyboard selection in the Graph tab and the node set it belongs to (§77).
    graph_selected: Option<NoteId>,
    graph_selection_key: String,
    search_mode: SearchMode,
    /// Selected Search result and the query/mode it belongs to (§64).
    search_selected: usize,
    search_selection_key: (String, SearchMode),
    /// Selected Health issue and the report identity it belongs to (§72).
    health_selected: usize,
    health_selection_key: String,
    monitor: DiskMonitor,
    last_poll: Instant,
    /// (note id, disk content, deleted-from-disk). The third field is
    /// explicit: an empty file is not the same as a missing one (§84 review).
    conflict: Option<(String, String, bool)>,
    /// §PROD-P4: pause autosave retries briefly after a conflict, so a
    /// dirty note does not re-read the disk on every frame (~60 Hz).
    last_conflict_at: Option<Instant>,
    last_saved: Option<(String, Instant)>,
    status: String,
    error: Option<String>,
}

fn app_data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(|profile| PathBuf::from(profile).join("AppData").join("Roaming"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Nexora")
}

fn semantic_index_path() -> PathBuf {
    app_data_dir().join("semantic.idx")
}

fn canvas_path() -> PathBuf {
    app_data_dir().join("canvas.idx")
}

fn base_path() -> PathBuf {
    app_data_dir().join("base.idx")
}

fn bookmarks_path() -> PathBuf {
    app_data_dir().join("bookmarks.idx")
}

fn theme_path() -> PathBuf {
    app_data_dir().join("theme.idx")
}

fn templates_path() -> PathBuf {
    app_data_dir().join("templates.idx")
}

fn saved_queries_path() -> PathBuf {
    app_data_dir().join("saved_queries.idx")
}

fn workspaces_path() -> PathBuf {
    app_data_dir().join("workspaces.idx")
}

fn named_bases_path() -> PathBuf {
    app_data_dir().join("named_bases.idx")
}

/// All app-data sidecars surface a load error once and fall back to a safe
/// default; the mechanism lives here so they cannot drift apart.
fn sidecar_or<T>(result: Result<T, NexoraError>, error: &mut Option<String>, fallback: T) -> T {
    match result {
        Ok(value) => value,
        Err(load_error) => {
            if error.is_none() {
                *error = Some(load_error.to_string());
            }
            fallback
        }
    }
}

impl NexoraApp {
    fn new() -> Self {
        let root = vault::default_vault_root();
        let (index, mut error, index_unavailable) = match vault::build_index(&root) {
            Ok(index) => (index, None, false),
            Err(error) => (VaultIndex::default(), Some(error.to_string()), true),
        };
        let mut monitor = DiskMonitor::new(&root);
        // §PROD-P3: the startup fingerprint error is surfaced once, unless
        // build_index already reported the same I/O failure.
        if let Some(fingerprint_error) = monitor.take_startup_error() {
            if error.is_none() {
                error = Some(fingerprint_error.to_string());
            }
        }
        // §83: all app-data sidecars surface a load error once; a corrupt file
        // must not be silently reset and then overwritten by the next write.
        let stored_canvas = sidecar_or(
            canvas::load(&canvas_path()).map(Some),
            &mut error,
            None,
        );
        let canvas = canvas::reconcile(stored_canvas, &index);
        let canvas_path = canvas_path();
        let base_path = base_path();
        let base_config = sidecar_or(bases::load(&base_path), &mut error, BaseConfig::default());
        let bookmarks_path = bookmarks_path();
        let mut bookmarks = sidecar_or(
            bookmarks::load(&bookmarks_path),
            &mut error,
            Bookmarks::default(),
        );
        bookmarks.prune(&index);
        let theme_path = theme_path();
        // A corrupt sidecar is surfaced, never silently reset (§35.3).
        let theme = sidecar_or(theme::load(&theme_path), &mut error, ThemePreset::Dark);
        let templates_path = templates_path();
        let templates = sidecar_or(
            templates::load(&templates_path),
            &mut error,
            vec![templates::builtin()],
        );
        let saved_queries = sidecar_or(
            saved_queries::load(&saved_queries_path()),
            &mut error,
            Vec::new(),
        );
        let workspaces = sidecar_or(workspace::load(&workspaces_path()), &mut error, Vec::new());
        let named_bases = sidecar_or(
            named_bases::load(&named_bases_path()),
            &mut error,
            Vec::new(),
        );
        let semantic_path = semantic_index_path();
        let (semantic_index, semantic_stale) = match semantic::load(&semantic_path) {
            Ok(index_on_disk) => {
                let stale = semantic::is_stale(&index_on_disk, &index);
                (Some(index_on_disk), stale)
            }
            Err(_) => (None, false),
        };
        Self {
            root,
            index,
            index_unavailable,
            recovery_notice: false,
            tabs: Vec::new(),
            active: 0,
            search_query: String::new(),
            tag_filter: None,
            collapsed: BTreeSet::new(),
            palette_open: false,
            palette_query: String::new(),
            palette_selected: 0,
            palette_selection_key: String::new(),
            capture_open: false,
            capture_text: String::new(),
            rename_input_open: false,
            rename_confirm_open: false,
            rename_new: String::new(),
            rename_plan: None,
            snapshot_modal: None,
            pending_line: None,
            quick_open: false,
            quick_query: String::new(),
            quick_results: Vec::new(),
            quick_selected: 0,
            outline_open: false,
            outline_query: String::new(),
            outline_results: Vec::new(),
            outline_selected: 0,
            properties_open: false,
            properties_note: None,
            prop_created: String::new(),
            prop_modified: String::new(),
            prop_aliases: String::new(),
            prop_tags: String::new(),
            link_popup: None,
            link_dismissed: None,
            selected_broken: None,
            markdown_cache: CommonMarkCache::default(),
            semantic_index,
            related_cache: None,
            semantic_path,
            semantic_stale,
            canvas_path,
            canvas,
            canvas_drag: None,
            canvas_selected: None,
            canvas_selection_key: String::new(),
            graph_selected: None,
            graph_selection_key: String::new(),
            base_path,
            base_sort_input: base_config.sort_field.clone(),
            base_config,
            base_column_input: String::new(),
            base_edit: None,
            base_selected: None,
            base_selection_key: String::new(),
            base_cards: false,
            bookmarks_path,
            bookmarks,
            bookmarks_open: false,
            theme_path,
            theme,
            theme_dirty: false,
            templates,
            template_open: false,
            template_picker_mode: TemplatePickerMode::NewNote,
            template_insert_note: None,
            template_editor_open: false,
            template_edit: None,
            template_name: String::new(),
            template_body: String::new(),
            template_delete: None,
            composer_mode: None,
            extract_first: "1".to_string(),
            extract_last: "1".to_string(),
            extract_new: String::new(),
            merge_targets: Vec::new(),
            saved_queries,
            saved_query_selected: 0,
            saved_query_selection_key: String::new(),
            query_name: String::new(),
            saved_query_rename: None,
            saved_query_delete: None,
            save_query_open: false,
            apply_query_open: false,
            workspaces,
            workspace_name: String::new(),
            workspace_rename: None,
            workspace_delete: None,
            save_workspace_open: false,
            apply_workspace_open: false,
            named_bases,
            base_view_name: String::new(),
            base_view_rename: None,
            base_view_delete: None,
            save_base_view_open: false,
            apply_base_view_open: false,
            review_open: false,
            grade_open: false,
            grade_note: None,
            review_rows: Vec::new(),
            review_selected: 0,
            search_mode: SearchMode::Keyword,
            search_selected: 0,
            search_selection_key: (String::new(), SearchMode::Keyword),
            health_selected: 0,
            health_selection_key: String::new(),
            monitor,
            last_poll: Instant::now(),
            conflict: None,
            last_conflict_at: None,
            last_saved: None,
            status: "Ready".to_string(),
            error,
        }
    }

    fn current_note_id(&self) -> Option<NoteId> {
        self.tabs
            .get(self.active)
            .filter(|tab| tab.kind == TabKind::Note)
            .map(|tab| NoteId::new(tab.id.clone()))
    }

    /// Returns `true` when the index was rebuilt successfully (§86).
    fn reload_index(&mut self) -> bool {
        let root = self.root.clone();
        let recovering = self.index_unavailable;
        let mut rebuilt = false;
        match vault::build_index(&root) {
            Ok(index) => {
                self.index = index;
                self.index_unavailable = false;
                rebuilt = true;
                // Applied at frame end so a write path cannot overwrite it
                // (§86 review).
                if recovering {
                    self.recovery_notice = true;
                    self.error = None;
                }
                // Do not clear a modal's inline error here: an autosave of an
                // unrelated tab must not make a failed write look successful
                // (§GUI-4). Successful writes clear it in their own path.
                let stored = Some(std::mem::take(&mut self.canvas));
                self.canvas = canvas::reconcile(stored, &self.index);
                let before = self.bookmarks.ids.len();
                self.bookmarks.prune(&self.index);
                if self.bookmarks.ids.len() != before {
                    self.save_bookmarks();
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        self.semantic_stale = self
            .semantic_index
            .as_ref()
            .is_some_and(|index| semantic::is_stale(index, &self.index));
        // Every Nexora write goes through reload_index; acknowledging here
        // prevents our own autosave from looking like an external edit.
        self.monitor.resync();
        self.related_cache = None;
        rebuilt
    }

    /// Bring every tab for a note in sync with content just written to disk.
    /// One owner prevents the write paths from drifting apart (§53 review).
    fn refresh_tab_after_write(&mut self, id: &NoteId, content: &str) {
        for tab in self.tabs.iter_mut().filter(|tab| {
            tab.kind == TabKind::Note && tab.id == id.as_str()
        }) {
            tab.content = content.to_string();
            tab.loaded = content.to_string();
            tab.dirty = false;
            tab.last_edit = None;
        }
        self.last_saved = Some((id.as_str().to_string(), Instant::now()));
    }

    /// One owner for GM-4 recovery after any guarded note write (§54).
    fn record_conflict(&mut self, id: &NoteId, path: &std::path::Path) {
        self.last_conflict_at = Some(Instant::now());
        let root = self.root.clone();
        match vault::load_note(&root, id) {
            Ok(note) => {
                self.conflict = Some((id.as_str().to_string(), note.content, false));
                self.error = None;
                self.status = format!("Conflict: {}", path.display());
            }
            // A conflict banner whose "disk content" is fabricated from an
            // empty default would make Reload destroy real data (§54 review).
            Err(error) => {
                self.conflict = None;
                self.error = Some(error.to_string());
                self.status = format!("Conflict: {} — disk unreadable", path.display());
            }
        }
    }

    /// Flush one note tab. `loaded_content` is the conflict-detection anchor.
    fn flush_tab(&mut self, idx: usize) {
        let should_write = self
            .tabs
            .get(idx)
            .is_some_and(|tab| tab.kind == TabKind::Note && tab.dirty);
        if !should_write {
            return;
        }
        let root = self.root.clone();
        let (id, content, loaded) = {
            let tab = &self.tabs[idx];
            (
                NoteId::new(tab.id.clone()),
                tab.content.clone(),
                tab.loaded.clone(),
            )
        };
        match save::write_note(&root, &id, &content, &loaded) {
            Ok(()) => {
                self.last_conflict_at = None;
                self.error = None;
                self.refresh_tab_after_write(&id, &content);
                self.status = format!("Saved {}", id);
                self.reload_index();
            }
            Err(NexoraError::Conflict { path }) => self.record_conflict(&id, &path),
            Err(error) => {
                self.error = Some(error.to_string());
                self.status = "Save failed".to_string();
            }
        }
    }

    fn autosave(&mut self) {
        let now = Instant::now();
        let due: Vec<usize> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| {
                // §PROD-P4: a conflicted note stays dirty on purpose; do not
                // re-read the disk on every frame until the cooldown lapses.
                let cooling_down = self
                    .conflict
                    .as_ref()
                    .is_some_and(|(id, _, _)| *id == tab.id)
                    && self
                        .last_conflict_at
                        .is_some_and(|at| now.duration_since(at) < CONFLICT_RETRY_COOLDOWN);
                tab.kind == TabKind::Note
                    && tab.dirty
                    && !cooling_down
                    && tab.last_edit.is_some_and(|at| now.duration_since(at) >= AUTOSAVE_IDLE)
            })
            .map(|(idx, _)| idx)
            .collect();
        for idx in due {
            self.flush_tab(idx);
        }
    }

    /// Compare disk state with what we last acknowledged. Clean tabs follow
    /// the disk; dirty tabs raise the GM-4 conflict banner instead of losing
    /// keystrokes. `ARCHITECTURE.md` §16.
    fn poll_disk(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_poll) < Duration::from_millis(1000) {
            return;
        }
        self.last_poll = now;
        let changes = match self.monitor.refresh() {
            Ok(changes) => changes,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        // §86: recovery may arrive with an empty change list (the failed
        // startup fingerprint can already contain the notes), so the retry
        // must run before this early return. Only a startup failure arms it.
        if self.index_unavailable {
            self.reload_index();
        }
        if changes.is_empty() {
            return;
        }
        let root = self.root.clone();
        let mut reload_needed = false;
        for change in changes {
            match change {
                Change::Created(_) => reload_needed = true,
                Change::Modified(id) => {
                    let disk = match vault::load_note(&root, &id) {
                        Ok(note) => note,
                        Err(error) => {
                            self.error = Some(error.to_string());
                            continue;
                        }
                    };
                    let dirty = self
                        .tabs
                        .iter()
                        .find(|tab| tab.kind == TabKind::Note && tab.id == id.as_str())
                        .is_some_and(|tab| tab.dirty);
                    if dirty {
                        if self.conflict.is_none() {
                            self.conflict = Some((
                                id.as_str().to_string(),
                                disk.content.clone(),
                                false,
                            ));
                            self.last_conflict_at = Some(Instant::now());
                            self.status = format!("External change in {id}");
                        }
                    } else if let Some(tab) = self
                        .tabs
                        .iter_mut()
                        .find(|tab| tab.kind == TabKind::Note && tab.id == id.as_str())
                    {
                        tab.content = disk.content.clone();
                        tab.loaded = disk.content;
                        tab.dirty = false;
                        tab.last_edit = None;
                        self.last_saved = None;
                    }
                    reload_needed = true;
                }
                Change::Deleted(id) => {
                    let tab_index = self.tabs.iter().position(|tab| {
                        tab.kind == TabKind::Note && tab.id == id.as_str()
                    });
                    match tab_index {
                        Some(idx) if self.tabs[idx].dirty => {
                            if self.conflict.is_none() {
                                self.conflict =
                                    Some((id.as_str().to_string(), String::new(), true));
                                self.last_conflict_at = Some(Instant::now());
                                self.status = format!("External deletion of {id}");
                            }
                        }
                        Some(idx) => {
                            self.tabs.remove(idx);
                            // Removing a tab to the left of the active one must
                            // shift the index, otherwise a different note opens.
                            if self.active >= idx && self.active > 0 {
                                self.active -= 1;
                            }
                            if self.active >= self.tabs.len() {
                                self.active = self.tabs.len().saturating_sub(1);
                            }
                        }
                        None => {}
                    }
                    reload_needed = true;
                }
            }
        }
        if reload_needed {
            self.reload_index();
        }
    }

    /// Grade the active note through the normal frontmatter/snapshot path (§46).
    /// Returns `true` on success; on failure `self.error` holds the reason and
    /// the caller keeps the modal open (§48/§78).
    fn apply_grade(&mut self, quality: u8) -> bool {
        // Pin the note at modal open; a tab click behind the window must not
        // redirect the grade (§GUI-3).
        let Some(pinned) = self.grade_note.clone() else {
            self.error = Some("No note open for grading".to_string());
            return false;
        };
        let id = NoteId::new(pinned);
        // A grade is computed from the saved index version. Grading over
        // unsaved buffer text would silently discard the user's edits and
        // leave a stale `loaded` anchor for autosave (§53).
        if self.tabs.iter().any(|tab| {
            tab.kind == TabKind::Note && tab.id == id.as_str() && tab.dirty
        }) {
            self.status = "Save or discard changes before grading".to_string();
            self.error = Some(self.status.clone());
            return false;
        }
        let date = review::today();
        let content = self
            .index
            .notes
            .get(&id)
            .map(|note| note.content.clone())
            .unwrap_or_default();
        let state = match review::from_content(&content) {
            Some(state) => state,
            None if review::has_fields(&content) => {
                // Malformed review state is data, not a fresh card: do not
                // silently overwrite it (§2).
                self.error = Some("Malformed review frontmatter".to_string());
                return false;
            }
            None => review::new_card(date),
        };
        let next = review::grade(&state, quality, date);
        let updated = review::write_state(&content, &next);
        match save::write_note(&self.root, &id, &updated, &content) {
            Ok(()) => {
                self.reload_index();
                // Every Nexora write refreshes its tab; grading must too, or
                // the editor keeps the old frontmatter and our own write
                // looks like an external change (§53).
                self.refresh_tab_after_write(&id, &updated);
                self.status = format!("Graded {id} · next due {}", next.due);
                true
            }
            // A grade is a normal note write: conflicts get the same GM-4
            // Reload/Overwrite recovery as autosave (§53 review).
            Err(NexoraError::Conflict { path }) => {
                self.record_conflict(&id, &path);
                self.error = Some("External change detected; reload or overwrite".to_string());
                false
            }
            Err(error) => { self.error = Some(error.to_string()); false }
        }
    }

    /// Jump the editor to a heading/block anchor after opening a note (§38.1).
    /// Unknown anchors open the note and leave the cursor alone.
    fn jump_to_anchor(&mut self, id: &NoteId, anchor: &str) {
        let content = self
            .index
            .notes
            .get(id)
            .map(|note| note.content.clone())
            .unwrap_or_default();
        match outline::line_for_anchor(&content, anchor) {
            Some(line) => {
                self.pending_line = Some((id.clone(), line));
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.view = ViewMode::Source;
                }
            }
            None => self.status = format!("Anchor not found: {anchor}"),
        }
    }

    fn open_note(&mut self, id: &NoteId) {
        if let Some(pos) = self
            .tabs
            .iter()
            .position(|tab| tab.kind == TabKind::Note && tab.id == id.as_str())
        {
            self.active = pos;
            return;
        }
        // §UX-P2c: a note deleted within the 1 Hz poll window has no index
        // entry; opening a tab for it silently yields an empty editor.
        let Some((title, content)) = self.index.notes.get(id).map(|note| {
            (note.title.clone(), note.content.clone())
        }) else {
            self.status = format!("Note no longer exists: {id}");
            return;
        };
        self.tabs.push(Tab {
            id: id.as_str().to_string(),
            title,
            kind: TabKind::Note,
            view: if content.trim().is_empty() {
                ViewMode::Source
            } else {
                ViewMode::Reading
            },
            content: content.clone(),
            loaded: content,
            dirty: false,
            last_edit: None,
        });
        self.active = self.tabs.len() - 1;
    }

    /// Turn `![[pic.png]]` into a local `bytes://` image URI and register the
    /// bytes with egui. Missing or unsafe targets get a visible marker (§45).
    fn prepare_images(&self, ctx: &egui::Context, content: &str) -> String {
        let mut out = content.to_string();
        for inner in images::all_targets(content) {
            let target = inner.split('|').next().unwrap_or("").trim();
            let original = format!("![[{inner}]]");
            let mut replacement = format!("{original} (image not found)");
            if images::is_safe_target(target) {
                let path = self.root.join(target);
                if let Ok(bytes) = fs::read(&path) {
                    // DefaultBytesLoader keeps the first bytes for a URI;
                    // version by mtime so a replaced image is not stale.
                    let version = fs::metadata(&path)
                        .ok()
                        .and_then(|meta| meta.modified().ok())
                        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                        .map(|elapsed| elapsed.as_nanos().to_string())
                        .unwrap_or_else(|| "0".to_string());
                    let uri = format!("bytes://nexora/{}?v={version}", encode_component(target));
                    ctx.include_bytes(uri.clone(), bytes);
                    replacement = format!("![{target}]({uri})");
                }
            }
            // One occurrence per entry: repeated embeds get one replacement
            // each, and aliases are matched exactly.
            out = out.replacen(&original, &replacement, 1);
        }
        out
    }

    fn open_special(&mut self, kind: TabKind) {
        let title = match kind {
            TabKind::Graph => "Graph",
            TabKind::Canvas => "Canvas",
            TabKind::Base => "Base",
            TabKind::Health => "Health",
            TabKind::Search => "Search",
            TabKind::Note => return,
        };
        if let Some(pos) = self.tabs.iter().position(|tab| tab.kind == kind) {
            self.active = pos;
            return;
        }
        self.tabs.push(Tab {
            id: format!("__{:?}__", kind),
            title: title.to_string(),
            kind,
            view: ViewMode::Source,
            content: String::new(),
            loaded: String::new(),
            dirty: false,
            last_edit: None,
        });
        self.active = self.tabs.len() - 1;
    }

    fn close_tab(&mut self, idx: usize) {
        self.flush_tab(idx);
        // §79: a failed flush leaves the tab dirty (conflict or I/O error).
        // Closing it then would delete the user's only buffer and make the
        // GM-4 banner unreachable, so abort and keep it open.
        if self.tabs.get(idx).is_some_and(|tab| tab.dirty) {
            self.status = "Save or discard changes before closing tab".to_string();
            return;
        }
        if idx >= self.tabs.len() {
            return;
        }
        self.tabs.remove(idx);
        if self.active >= idx && self.active > 0 {
            self.active -= 1;
        }
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
    }

    fn unused_note_id(&self, base: &str) -> NoteId {
        let mut number = 1;
        loop {
            let candidate = if number == 1 {
                base.to_string()
            } else {
                format!("{base}-{number}")
            };
            let id = NoteId::new(candidate);
            if !self.index.notes.contains_key(&id) {
                return id;
            }
            number += 1;
        }
    }

    fn create_note(&mut self) {
        let id = self.unused_note_id("Untitled");
        self.write_new_note(&id, String::new());
    }

    fn write_new_note(&mut self, id: &NoteId, body: String) -> bool {
        let root = self.root.clone();
        match save::write_note(&root, id, &body, "") {
            Ok(()) => {
                self.last_saved = Some((id.as_str().to_string(), Instant::now()));
                self.reload_index();
                self.open_note(id);
                true
            }
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }

    /// Create a note from a rendered template. Never overwrites an existing
    /// note: the id is checked against the live index first (§36.1). The
    /// shipped daily template gets a date-named note, so both placeholders
    /// resolve to a real day.
    fn create_from_template(&mut self, template: &Template) -> bool {
        let date = daily::today_id();
        let id = self.unused_note_id(date.as_str());
        let fields = templates::Fields {
            date: date.as_str(),
            title: id.as_str(),
        };
        let body = templates::render(&template.body, &fields);
        if self.write_new_note(&id, body) {
            self.status = format!("Created {} from {}", id, template.name);
            true
        } else {
            // §81: keep the picker open so the inline write error stays visible.
            false
        }
    }

    /// Insert a rendered template at the Source editor's caret. Returns
    /// `false` when the picker must stay open (dirty tab or failed write) (§75).
    fn insert_template(&mut self, ctx: &egui::Context, template: &Template) -> bool {
        // The picker is not egui-modal: use the note pinned at open time
        // instead of the tab that may have been clicked behind it (§GUI-3).
        let pinned = self.template_insert_note.clone();
        let (id_string, dirty) = match pinned.and_then(|pinned| {
            self.tabs
                .iter()
                .find(|tab| tab.kind == TabKind::Note && tab.id == pinned)
                .map(|tab| (tab.id.clone(), tab.dirty))
        }) {
            Some(found) => found,
            None => {
                let message = "Open a note in Source mode first".to_string();
                self.status = message.clone();
                self.error = Some(message);
                return false;
            }
        };
        if dirty {
            // §48: the picker stays open, so the reason must be inline too.
            let message = "Save or discard changes before inserting a template".to_string();
            self.status = message.clone();
            self.error = Some(message);
            return false;
        }
        let id = NoteId::new(id_string.clone());
        let title = self
            .index
            .notes
            .get(&id)
            .map(|note| note.title.clone())
            .unwrap_or_else(|| id.as_str().to_string());
        let Some(disk) = self.index.notes.get(&id).map(|note| note.content.clone()) else {
            let message = format!("Note no longer exists: {id}");
            self.status = message.clone();
            self.error = Some(message);
            return false;
        };
        let editor_id = egui::Id::unique(("nexora_editor", id_string.clone()));
        let char_count = disk.chars().count();
        let cursor = egui::TextEdit::load_state(ctx, editor_id.clone())
            .and_then(|state| state.cursor.char_range())
            .map(|range| range.primary.index.into())
            .unwrap_or(char_count)
            .min(char_count);
        let date = daily::today_id();
        let body = templates::render(
            &template.body,
            &templates::Fields {
                date: date.as_str(),
                title: &title,
            },
        );
        let edit = templates::insert_at(&disk, cursor, &body);
        match save::write_note(&self.root, &id, &edit.text, &disk) {
            Ok(()) => {
                self.reload_index();
                self.refresh_tab_after_write(&id, &edit.text);
                set_editor_cursor(ctx, editor_id, edit.cursor);
                self.error = None;
                self.status = format!("Inserted {} from {}", template.name, id);
                true
            }
            Err(NexoraError::Conflict { path }) => {
                let message = NexoraError::Conflict { path: path.clone() }.to_string();
                self.record_conflict(&id, &path);
                self.error = Some(message);
                false
            }
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }

    fn capture(&mut self) {
        let text = self.capture_text.trim().to_string();
        if text.is_empty() {
            self.status = "Capture text is empty".to_string();
            return;
        }
        // GM-4: refreshing a dirty Inbox tab would silently destroy its
        // unsaved buffer. Refuse instead, like the other guarded writes.
        let inbox_id = NoteId::new(INBOX_ID);
        if self.tabs.iter().any(|tab| {
            tab.kind == TabKind::Note && tab.id == inbox_id.as_str() && tab.dirty
        }) {
            self.status = "Save or discard changes before capturing".to_string();
            self.error = Some(self.status.clone());
            return;
        }
        let root = self.root.clone();
        match inbox::capture(&root, &text) {
            Ok(()) => {
                let inbox = NoteId::new(INBOX_ID);
                self.reload_index();
                // An already-open Inbox tab returns early from open_note;
                // refresh its buffer first or a later autosave would restore
                // the pre-capture content (§GUI-1).
                let content = self
                    .index
                    .notes
                    .get(&inbox)
                    .map(|note| note.content.clone());
                if let Some(content) = content {
                    self.refresh_tab_after_write(&inbox, &content);
                }
                self.open_note(&inbox);
                self.capture_text.clear();
                self.capture_open = false;
                self.last_saved = Some((INBOX_ID.to_string(), Instant::now()));
                self.status = "Captured to Inbox".to_string();
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn open_snapshot(&mut self, stamp: &str) {
        let Some(id) = self.current_note_id() else {
            return;
        };
        // §48: a modal must not display an unrelated earlier failure.
        self.error = None;
        let root = self.root.clone();
        match save::read_snapshot(&root, &id, stamp) {
            Ok(old) => {
                let current = self
                    .tabs
                    .get(self.active)
                    .map(|tab| tab.content.clone())
                    .unwrap_or_default();
                // §54: a restore must not hide any unsaved buffer for the
                // note, not just the active tab's buffer.
                let dirty = self.tabs.iter().any(|tab| {
                    tab.kind == TabKind::Note && tab.id == id.as_str() && tab.dirty
                });
                self.snapshot_modal = Some(SnapshotModal {
                    id,
                    stamp: stamp.to_string(),
                    rows: diff(&old, &current),
                    dirty,
                });
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn open_newest_snapshot(&mut self) {
        let Some(id) = self.current_note_id() else {
            self.status = "No note open".to_string();
            return;
        };
        let root = self.root.clone();
        match save::list_snapshots(&root, &id) {
            Ok(stamps) => match stamps.first() {
                Some(stamp) => self.open_snapshot(stamp),
                None => self.status = "No snapshots for this note".to_string(),
            },
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn confirm_restore(&mut self) {
        let Some(mut modal) = self.snapshot_modal.take() else {
            return;
        };
        // Re-check at commit time: another tab may have become dirty while
        // the modal was open (§54). Keep the modal's own warning in sync,
        // otherwise it reopens with Restore still enabled and no explanation.
        let dirty = self.tabs.iter().any(|tab| {
            tab.kind == TabKind::Note && tab.id == modal.id.as_str() && tab.dirty
        });
        if dirty || modal.dirty {
            modal.dirty = true;
            self.snapshot_modal = Some(modal);
            self.status = "Save or discard changes before restoring".to_string();
            return;
        }
        let root = self.root.clone();
        match save::restore_snapshot(&root, &modal.id, &modal.stamp) {
            Ok(restored) => {
                self.reload_index();
                // Refresh from the bytes we actually wrote, not from the
                // index, which may lag behind on a failed reload (§54).
                self.refresh_tab_after_write(&modal.id, &restored);
                self.status = format!("Restored {}", modal.stamp);
            }
            Err(NexoraError::Conflict { path }) => {
                self.record_conflict(&modal.id, &path);
                self.snapshot_modal = Some(modal);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                self.snapshot_modal = Some(modal);
            }
        }
    }

    fn open_properties(&mut self) {
        let Some(id) = self.current_note_id() else {
            self.status = "No note open".to_string();
            return;
        };
        // §48: keep this modal's inline error local to this modal.
        self.error = None;
        self.properties_note = Some(id.as_str().to_string());
        let content = self
            .index
            .notes
            .get(&id)
            .map(|note| note.content.clone())
            .unwrap_or_default();
        self.prop_created = read_field(&content, "created").unwrap_or_default();
        self.prop_modified = read_field(&content, "modified").unwrap_or_default();
        self.prop_aliases = read_list(&content, "aliases").join(", ");
        self.prop_tags = read_list(&content, "tags").join(", ");
        self.properties_open = true;
    }

    fn save_properties(&mut self) {
        // The window is not egui-modal: commit to the note it was opened for,
        // never to whichever tab is active now (§GUI-3).
        let Some(pinned) = self.properties_note.clone() else {
            return;
        };
        let id = NoteId::new(pinned);
        // §81: Properties edit the saved version, just like grading. A dirty
        // buffer must be saved or discarded first, and the write anchors on
        // `tab.loaded` (disk-as-seen-at-load), never on the dirty buffer —
        // otherwise write_note raises a false conflict whose Reload would
        // destroy the user's keystrokes.
        if self.tabs.iter().any(|tab| {
            tab.kind == TabKind::Note && tab.id == id.as_str() && tab.dirty
        }) {
            let message = "Save or discard changes before editing properties".to_string();
            self.status = message.clone();
            self.error = Some(message);
            return;
        }
        // The tab strip stays clickable behind egui windows, so the source
        // buffer must be found by the pinned id, not by `self.active` (§87).
        let (loaded, mut content) = match self.tabs.iter().find(|tab| {
            tab.kind == TabKind::Note && tab.id == id.as_str()
        }) {
            Some(tab) => (tab.loaded.clone(), tab.content.clone()),
            None => return,
        };
        for (key, value) in [("created", self.prop_created.clone()), ("modified", self.prop_modified.clone())] {
            content = write_field(&content, key, &value);
        }
        let aliases: Vec<String> = self
            .prop_aliases
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect();
        let tags: Vec<String> = self
            .prop_tags
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect();
        content = write_list(&content, "aliases", &aliases);
        content = write_list(&content, "tags", &tags);

        let root = self.root.clone();
        match save::write_note(&root, &id, &content, &loaded) {
            Ok(()) => {
                self.reload_index();
                // §54: one refresh owner for every write to an existing note.
                self.refresh_tab_after_write(&id, &content);
                self.properties_open = false;
                self.error = None;
                self.status = "Properties saved".to_string();
            }
            Err(NexoraError::Conflict { path }) => self.record_conflict(&id, &path),
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn open_quick_switcher(&mut self) {
        self.quick_query.clear();
        self.refresh_quick();
        self.quick_open = true;
    }

    fn alias_score(&self, query: &str, id: &NoteId) -> Option<u32> {
        self.index
            .aliases
            .iter()
            .filter(|(_, target)| *target == id)
            .filter_map(|(alias, _)| fuzzy_score(query, alias))
            .max()
    }

    fn refresh_quick(&mut self) {
        let query = self.quick_query.clone();
        let mut ranked: Vec<(u32, NoteId)> = self
            .index
            .notes
            .iter()
            .filter_map(|(id, note)| {
                let title = fuzzy_score(&query, &note.title).unwrap_or(0);
                let path = fuzzy_score(&query, id.as_str()).unwrap_or(0);
                let alias = self.alias_score(&query, id).unwrap_or(0);
                let value = title.max(path).max(alias);
                (query.trim().is_empty() || value > 0).then(|| (value, id.clone()))
            })
            .collect();
        ranked.sort_by(|left, right| {
            right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1))
        });
        self.quick_results = ranked.into_iter().take(20).map(|(_, id)| id).collect();
        self.quick_selected = 0;
    }

    fn open_outline(&mut self) {
        self.outline_query.clear();
        self.refresh_outline();
        self.outline_open = true;
    }

    fn refresh_outline(&mut self) {
        let Some(id) = self.current_note_id() else {
            self.outline_results.clear();
            self.status = "No note open".to_string();
            return;
        };
        let content = self
            .index
            .notes
            .get(&id)
            .map(|note| note.content.clone())
            .unwrap_or_default();
        let query = self.outline_query.clone();
        self.outline_results = extract_headings(&content)
            .into_iter()
            .filter(|heading| {
                query.trim().is_empty() || fuzzy_score(&query, &heading.title).is_some()
            })
            .take(50)
            .map(|heading| (id.clone(), heading.line, heading.title))
            .collect();
        self.outline_selected = 0;
    }

    fn build_semantic_index(&mut self) {
        let root = self.root.clone();
        let index = match vault::build_index(&root) {
            Ok(index) => index,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let built = semantic::build(&index, DEFAULT_DIMS);
        let count = built.entries.len();
        let path = self.semantic_path.clone();
        if let Some(parent) = path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                self.error = Some(NexoraError::io_at(parent, error).to_string());
                return;
            }
        }
        match semantic::save(&path, &built) {
            Ok(()) => {
                self.semantic_index = Some(built);
                self.related_cache = None;
                self.semantic_stale = false;
                self.status = format!("Vector index: {count} chunks");
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn begin_rename(&mut self) {
        let Some(id) = self.current_note_id() else {
            self.status = "No note open".to_string();
            return;
        };
        self.rename_new = id.as_str().to_string();
        self.rename_plan = None;
        self.rename_input_open = true;
        self.rename_confirm_open = false;
    }

    fn preview_rename(&mut self) {
        let Some(old) = self.current_note_id() else {
            return;
        };
        let root = self.root.clone();
        let new = self.rename_new.clone();
        match rename::plan(&root, &old, &new) {
            Ok(plan) => {
                self.rename_plan = Some(plan);
                self.rename_input_open = false;
                self.rename_confirm_open = true;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn apply_rename(&mut self) {
        // §PROD-P2/UX-P1: a dirty tab must not be silently dropped by the
        // monitor (the disk has the pre-rename bytes). Refuse and keep the
        // plan; the user saves, re-previews and confirms again.
        let dirty = self.tabs.iter().any(|tab| {
            tab.kind == TabKind::Note
                && self
                    .rename_plan
                    .as_ref()
                    .is_some_and(|plan| tab.id == plan.old.as_str())
                && tab.dirty
        });
        if dirty {
            // Same shape as the other dirty guards (close_tab, grading,
            // restoring): status only, no duplicate error line (§UX-P3b).
            self.status = "Save or discard changes before renaming".to_string();
            return;
        }
        let Some(plan) = self.rename_plan.take() else {
            return;
        };
        let root = self.root.clone();
        match rename::apply(&root, &plan) {
            Ok(()) => {
                // Before rebuilding the index, remember which clean tabs show
                // notes whose text was rewritten by the rename.
                let rewritten: Vec<NoteId> = plan.edits.iter().map(|edit| edit.id.clone()).collect();
                // Rename bookmarks before the index rebuild prunes the old id.
                self.bookmarks.rename(&plan.old, &plan.new);
                self.save_bookmarks();
                self.reload_index();
                // The new path is only in the index after the rebuild;
                // reading its title earlier falls back to a bare stem (§87).
                let new_title = self
                    .index
                    .notes
                    .get(&plan.new)
                    .map(|note| note.title.clone())
                    .unwrap_or_else(|| plan.new.stem().to_string());
                let contents: Vec<(String, String)> = rewritten
                    .iter()
                    .filter_map(|id| {
                        self.index
                            .notes
                            .get(id)
                            .map(|note| (id.as_str().to_string(), note.content.clone()))
                    })
                    .collect();
                // `plan.edits` is keyed by the old id, so `contents` cannot
                // contain `plan.new`; read the renamed note directly.
                let renamed_content = self
                    .index
                    .notes
                    .get(&plan.new)
                    .map(|note| note.content.clone());
                for tab in &mut self.tabs {
                    if tab.kind != TabKind::Note {
                        continue;
                    }
                    if tab.id == plan.old.as_str() {
                        tab.id = plan.new.as_str().to_string();
                        tab.title = new_title.clone();
                        // A self-link `[[old]]` is rewritten on disk; the
                        // renamed tab must show the new text, not a stale
                        // buffer that looks clean (§GUI-5).
                        if let Some(content) = &renamed_content {
                            tab.content = content.clone();
                        }
                        tab.loaded = tab.content.clone();
                        tab.dirty = false;
                        tab.last_edit = None;
                    } else if !tab.dirty {
                        if let Some((_, content)) = contents.iter().find(|(id, _)| id == &tab.id) {
                            tab.content = content.clone();
                            tab.loaded = content.clone();
                        }
                    }
                }
                self.rename_confirm_open = false;
                self.last_saved = Some((plan.new.as_str().to_string(), Instant::now()));
                self.status = format!("Renamed to {}", plan.new);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn execute(&mut self, command: CommandId) {
        match command {
            CommandId::NewNote => self.create_note(),
            CommandId::EditProperties => self.open_properties(),
            CommandId::RenameNote => self.begin_rename(),
            CommandId::OpenToday => {
                let root = self.root.clone();
                let body = templates::daily_body(&self.templates);
                match daily::open_or_create_with(&root, &body) {
                    Ok(id) => {
                        self.reload_index();
                        self.open_note(&id);
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
            CommandId::QuickCapture => self.capture_open = true,
            CommandId::Search => self.open_special(TabKind::Search),
            CommandId::QuickOpen => self.open_quick_switcher(),
            CommandId::GoToHeading => self.open_outline(),
            CommandId::OpenGraph => self.open_special(TabKind::Graph),
            CommandId::OpenCanvas => self.open_special(TabKind::Canvas),
            CommandId::OpenBase => self.open_special(TabKind::Base),
            CommandId::ToggleBookmark => {
                if let Some(id) = self.current_note_id() {
                    let added = self.bookmarks.toggle(&id);
                    self.save_bookmarks();
                    self.status = if added {
                        format!("Bookmarked {id}")
                    } else {
                        format!("Removed bookmark {id}")
                    };
                } else {
                    self.status = "No note open".to_string();
                }
            }
            CommandId::ManageBookmarks => {
                self.error = None;
                self.bookmarks_open = true;
            }
            CommandId::CycleTheme => {
                self.theme = self.theme.next();
                self.theme_dirty = true;
                if let Err(error) = theme::save(&self.theme_path, self.theme) {
                    self.error = Some(error.to_string());
                } else {
                    self.status = format!("Theme: {}", self.theme.label());
                }
            }
            CommandId::NewFromTemplate => {
                if self.templates.is_empty() {
                    self.status = "No templates available".to_string();
                } else {
                    self.error = None;
                    self.template_delete = None;
                    self.template_picker_mode = TemplatePickerMode::NewNote;
                    self.template_open = true;
                }
            }
            CommandId::InsertTemplate => {
                let source_note = self.tabs.get(self.active).is_some_and(|tab| {
                    tab.kind == TabKind::Note && tab.view == ViewMode::Source
                });
                if !source_note {
                    self.status = "Open a note in Source mode first".to_string();
                } else if self.templates.is_empty() {
                    self.status = "No templates available".to_string();
                } else {
                    self.error = None;
                    self.template_delete = None;
                    self.template_insert_note = self.tabs.get(self.active).map(|tab| tab.id.clone());
                    self.template_picker_mode = TemplatePickerMode::Insert;
                    self.template_open = true;
                }
            }
            CommandId::ExtractLines => {
                if self.current_note_id().is_some() {
                    self.extract_new = format!("extracted-{}", self.index.notes.len() + 1);
                    // §48: a stale error from another modal must not appear
                    // inside this freshly opened one.
                    self.error = None;
                    self.composer_mode = Some(ComposerMode::Extract);
                } else {
                    self.status = "Open a note first".to_string();
                }
            }
            CommandId::MergeNote => {
                if let Some(current) = self.current_note_id() {
                    self.merge_targets = self
                        .index
                        .notes
                        .keys()
                        .filter(|id| **id != current)
                        .cloned()
                        .collect();
                    self.error = None;
                    if self.merge_targets.is_empty() {
                        // §UX-P2a: opening the modal with zero rows is a
                        // dead end; say so instead.
                        self.status = "Nothing to merge into.".to_string();
                    } else {
                        self.composer_mode = Some(ComposerMode::Merge);
                    }
                } else {
                    self.status = "Open a note first".to_string();
                }
            }
            CommandId::SaveQuery => {
                if self.search_query.trim().is_empty() {
                    self.status = "Type a query first".to_string();
                } else {
                    self.query_name.clear();
                    self.error = None;
                    self.save_query_open = true;
                }
            }
            CommandId::ApplySavedQuery => {
                if self.saved_queries.is_empty() {
                    self.status = "No saved queries yet".to_string();
                } else {
                    self.error = None;
                    self.saved_query_rename = None;
                    self.saved_query_delete = None;
                    self.saved_query_selected = 0;
                    self.saved_query_selection_key = saved_queries::selection_key(&self.saved_queries);
                    self.apply_query_open = true;
                }
            }
            CommandId::SaveWorkspace => {
                self.workspace_name.clear();
                self.error = None;
                self.save_workspace_open = true;
            }
            CommandId::ApplyWorkspace => {
                // §48: never render an unrelated earlier failure inline.
                self.error = None;
                let vault = self.root.to_string_lossy().to_string();
                if workspace::for_vault(&self.workspaces, &vault).is_empty() {
                    self.status = "No workspaces for this vault".to_string();
                } else {
                    self.workspace_rename = None;
                    self.workspace_delete = None;
                    self.apply_workspace_open = true;
                }
            }
            CommandId::SaveBaseView => {
                self.base_view_name.clear();
                self.error = None;
                self.save_base_view_open = true;
            }
            CommandId::ApplyBaseView => {
                self.error = None;
                if self.named_bases.is_empty() {
                    self.status = "No saved Base views yet".to_string();
                } else {
                    self.base_view_rename = None;
                    self.base_view_delete = None;
                    self.apply_base_view_open = true;
                }
            }
            CommandId::ReviewQueue => {
                self.review_rows = review::due_queue(&self.index, review::today());
                if self.review_rows.is_empty() {
                    self.status = "Nothing due today — add review_due: YYYY-MM-DD to frontmatter"
                        .to_string();
                } else {
                    self.review_selected = 0;
                    self.review_open = true;
                }
            }
            CommandId::GradeNote => {
                if let Some(id) = self.current_note_id() {
                    self.error = None;
                    self.grade_note = Some(id.as_str().to_string());
                    self.grade_open = true;
                } else {
                    self.status = "Open a note first".to_string();
                }
            }
            CommandId::HealthCheck => self.open_special(TabKind::Health),
            CommandId::RestoreSnapshot => self.open_newest_snapshot(),
            CommandId::BuildSemanticIndex => {
                self.build_semantic_index();
                self.open_special(TabKind::Search);
            }
            CommandId::CreateLinkTarget => {
                if let Some(target) = self.selected_broken.clone() {
                    let root = self.root.clone();
                    match save::write_note(&root, &target, "", "") {
                        Ok(()) => {
                            self.reload_index();
                            self.open_note(&target);
                            self.last_saved = Some((target.as_str().to_string(), Instant::now()));
                            self.status = format!("Created {}", target);
                            // §49: do not let a later palette invocation reuse
                            // a target that was already created.
                            self.selected_broken = None;
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                } else {
                    self.status = "Select a broken link first".to_string();
                }
            }
            CommandId::CloseTab => {
                let idx = self.active;
                self.close_tab(idx);
            }
        }
    }

    /// True while any modal owns keyboard input (§64).
    fn modal_open(&self) -> bool {
        self.palette_open
            || self.quick_open
            || self.capture_open
            || self.rename_input_open
            || self.rename_confirm_open
            || self.outline_open
            || self.properties_open
            || self.bookmarks_open
            || self.template_open
            || self.template_editor_open
            || self.composer_mode.is_some()
            || self.save_query_open
            || self.apply_query_open
            || self.save_workspace_open
            || self.apply_workspace_open
            || self.save_base_view_open
            || self.apply_base_view_open
            || self.review_open
            || self.grade_open
            || self.snapshot_modal.is_some()
    }

    fn read_keyboard(ctx: &egui::Context) -> Keyboard {
        ctx.input(|input| {
            let ctrl = input.modifiers.ctrl;
            let shift = input.modifiers.shift;
            Keyboard {
                quick_open: ctrl && !shift && input.key_pressed(Key::O),
                goto_heading: ctrl && shift && input.key_pressed(Key::O),
                rename: input.key_pressed(Key::F2),
                toggle_view: ctrl && input.key_pressed(Key::E),
                palette: ctrl && input.key_pressed(Key::P),
                new_note: ctrl && !shift && input.key_pressed(Key::N),
                capture: ctrl && shift && input.key_pressed(Key::N),
                search: ctrl && input.key_pressed(Key::F),
                close_tab: ctrl && input.key_pressed(Key::W),
                next_tab: ctrl && !shift && input.key_pressed(Key::Tab),
                prev_tab: ctrl && shift && input.key_pressed(Key::Tab),
                escape: input.key_pressed(Key::Escape),
            }
        })
    }

    /// Shared note row: hover preview + click-to-open. Used by panels whose
    /// scope includes previews (Related, Backlinks, Explorer, Bookmarks).
    /// The outgoing-links panel deliberately draws its own rows: §34 excludes
    /// hover previews there.
    fn note_row(&mut self, ui: &mut egui::Ui, label: String, id: &NoteId) {
        let preview = preview::for_note(&self.index, id, 8);
        let title = self
            .index
            .notes
            .get(id)
            .map(|note| note.title.clone())
            .unwrap_or_else(|| label.clone());
        let mut clicked = false;
        ui.vertical(|ui| {
            let response = ui.button(RichText::new(title).color(text()));
            clicked = response.clicked();
            hover_preview(ui, response, preview);
            ui.colored_label(muted(), RichText::new(&label).size(12.0));
        });
        if clicked {
            self.open_note(id);
        }
    }

    fn save_bookmarks(&mut self) {
        let path = self.bookmarks_path.clone();
        if let Err(error) = bookmarks::save(&path, &self.bookmarks) {
            self.error = Some(error.to_string());
        }
    }

    fn save_base_config(&mut self) {
        let path = self.base_path.clone();
        if let Err(error) = bases::save(&path, &self.base_config) {
            self.error = Some(error.to_string());
        }
    }

    /// Commit one Base cell through the same guarded write path as the editor.
    fn commit_base_edit(&mut self, edit: BaseEdit) {
        let id = edit.id.clone();
        let dirty = self.tabs.iter().any(|tab| {
            tab.kind == TabKind::Note && tab.id == id.as_str() && tab.dirty
        });
        if dirty {
            self.status = "Save or discard changes before editing in Base".to_string();
            // Keep the edit alive so the typed value is not lost.
            self.base_edit = Some(edit);
            return;
        }
        let Some(disk) = self.index.notes.get(&id).map(|note| note.content.clone()) else {
            self.status = format!("Note no longer exists: {id}");
            self.base_edit = None;
            return;
        };
        let updated = bases::edit_cell(&disk, &edit.key, &edit.value);
        let root = self.root.clone();
        match save::write_note(&root, &id, &updated, &disk) {
            Ok(()) => {
                self.base_edit = None;
                self.error = None;
                self.status = format!("Updated {} in {id}", edit.key);
                self.reload_index();
                self.refresh_tab_after_write(&id, &updated);
            }
            Err(NexoraError::Conflict { path }) => {
                self.base_edit = Some(edit);
                self.record_conflict(&id, &path);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                self.status = "Base edit failed".to_string();
                self.base_edit = Some(edit);
            }
        }
    }

    /// One cell of the Base table: a button that selects it, or a single-line
    /// editor with explicit OK/Cancel while selected (§56).
    /// Stable id scope for a Base cell button (§80 review). egui 0.37 Button
    /// has no `.id()`, so the cell gets its own push_id scope instead.
    fn base_cell_button_id(cell: &BaseCellRef) -> egui::Id {
        egui::Id::unique((
            "base_cell_button",
            cell.id.as_str(),
            cell.group.as_str(),
            cell.occurrence,
            cell.column.clone(),
            cell.key.clone(),
        ))
    }

    fn base_cell_ui(
        &mut self,
        ui: &mut egui::Ui,
        cell: &BaseCellRef,
        index: (usize, usize),
        selected: bool,
    ) -> BaseCellAction {
        let editing = self.base_edit.as_ref().is_some_and(|edit| {
            edit.id == cell.id
                && edit.group == cell.group
                && edit.key == cell.key
                && edit.column == cell.column
                && edit.occurrence == cell.occurrence
        });
        if !editing {
            let label = if cell.value.is_empty() { "—" } else { &cell.value };
            let button_id = Self::base_cell_button_id(cell);
            let mut button = egui::Button::new(RichText::new(label).color(text()));
            if selected {
                button = button.fill(raised());
            }
            let response = ui.push_id(button_id, |ui| ui.add(button)).inner;
            if response.clicked() {
                response.request_focus();
                self.base_selected = Some(index);
                self.base_edit = Some(BaseEdit {
                    id: cell.id.clone(),
                    group: cell.group.clone(),
                    occurrence: cell.occurrence,
                    column: cell.column.clone(),
                    key: cell.key.clone(),
                    value: cell.value.clone(),
                });
            }
            return BaseCellAction::None;
        }
        let mut current = self
            .base_edit
            .as_ref()
            .map(|edit| edit.value.clone())
            .unwrap_or_default();
        let editor_id = egui::Id::unique((
            "base_cell",
            cell.id.as_str(),
            cell.group.as_str(),
            cell.occurrence,
            cell.column.clone(),
            cell.key.clone(),
        ));
        // The editor appears one frame after the cell was clicked; without an
        // explicit focus request, typing would go to the previously focused
        // widget (possibly the note editor) and Enter could not commit (§56).
        if !ui.memory(|memory| memory.has_focus(editor_id)) {
            ui.memory_mut(|memory| memory.request_focus(editor_id));
        }
        let response = ui.add(
            egui::TextEdit::singleline(&mut current)
                .id(editor_id)
                .desired_width(90.0),
        );
        let enter = response.lost_focus() && input_enter(ui.ctx());
        let escape = ui.input(|input| input.key_pressed(Key::Escape));
        let mut action = BaseCellAction::None;
        if ui.small_button("OK").clicked() || enter {
            action = BaseCellAction::Commit;
        }
        if ui.small_button("×").clicked() || escape {
            action = BaseCellAction::Cancel;
        }
        self.base_edit = Some(BaseEdit {
            id: cell.id.clone(),
            group: cell.group.clone(),
            occurrence: cell.occurrence,
            column: cell.column.clone(),
            key: cell.key.clone(),
            value: current,
        });
        action
    }

    /// Bases-lite: frontmatter projection with filter/sort/group and single-cell edits.
    fn draw_base(&mut self, ui: &mut egui::Ui) {
        let tag_keys: Vec<String> = self.index.tags.keys().cloned().collect();
        let mut config_changed = false;
        let chip_columns = self.base_config.extra_columns.clone();
        let mut add_column = None;
        let mut remove_column = None;
        ui.horizontal(|ui| {
            ui.label("column");
            ui.add(
                egui::TextEdit::singleline(&mut self.base_column_input)
                    .id(egui::Id::unique("base_config_column"))
                    .hint_text("frontmatter key")
                    .desired_width(100.0),
            );
            if ui.button("Add").clicked() {
                add_column = Some(self.base_column_input.clone());
            }
            for key in &chip_columns {
                if ui.small_button(format!("{key} ×")).clicked() {
                    remove_column = Some(key.clone());
                    break;
                }
            }
        });
        if let Some(key) = add_column {
            if bases::add_column(&mut self.base_config, &key) {
                self.base_column_input.clear();
                config_changed = true;
            } else {
                self.error = Some("Invalid or duplicate Base column".to_string());
            }
        }
        if let Some(key) = remove_column {
            bases::remove_column(&mut self.base_config, &key);
            config_changed = true;
        }
        let extra_columns = self.base_config.extra_columns.clone();
        ui.horizontal(|ui| {
            ui.label("tag");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut self.base_config.filter_tag)
                        .id(egui::Id::unique("base_config_tag"))
                        .hint_text("all")
                        .desired_width(90.0),
                )
                .changed()
            {
                config_changed = true;
            }
            ui.separator();
            ui.label("sort");
            for option in [
                SortBy::Title,
                SortBy::Path,
                SortBy::Created,
                SortBy::Modified,
                SortBy::Field,
            ] {
                if ui
                    .selectable_label(self.base_config.sort_by == option, option.label())
                    .clicked()
                {
                    self.base_config.sort_by = option;
                    config_changed = true;
                }
            }
            let direction = if self.base_config.sort_desc { "desc" } else { "asc" };
            // §UX-P3e: bare tokens carry no state for screen readers; the
            // label says what it is and which direction is active.
            let direction_label = format!("Sort: {direction}");
            let direction_color = if self.base_config.sort_desc {
                accent()
            } else {
                muted()
            };
            if ui
                .add(egui::Button::new(
                    RichText::new(direction_label).color(direction_color),
                ))
                .clicked()
            {
                self.base_config.sort_desc = !self.base_config.sort_desc;
                config_changed = true;
            }
            ui.separator();
            ui.label("group");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut self.base_config.group_by)
                        .id(egui::Id::unique("base_config_group"))
                        .hint_text("none")
                        .desired_width(80.0),
                )
                .changed()
            {
                config_changed = true;
            }
            ui.separator();
            if ui.selectable_label(!self.base_cards, "table").clicked() {
                self.base_cards = false;
            }
            if ui.selectable_label(self.base_cards, "cards").clicked() {
                self.base_cards = true;
            }
        });
        let mut clear_filters_requested = false;
        let filters_active = bases::has_filters(&self.base_config);
        ui.horizontal(|ui| {
            ui.label("contains");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut self.base_config.filter_text)
                        .id(egui::Id::unique("base_config_contains"))
                        .hint_text("filter text")
                        .desired_width(140.0),
                )
                .changed()
            {
                config_changed = true;
            }
            if filters_active && ui.small_button("Clear filters").clicked() {
                clear_filters_requested = true;
            }
        });
        if self.base_config.sort_by == SortBy::Field {
            ui.horizontal(|ui| {
                ui.label("sort field");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.base_sort_input)
                            .id(egui::Id::unique("base_config_sort_field"))
                            .hint_text("frontmatter key")
                            .desired_width(120.0),
                    )
                    .changed()
                {
                    // Keep the raw draft for typing, but only commit a valid
                    // key to the config that is sorted on and persisted.
                    bases::set_sort_field(&mut self.base_config, &self.base_sort_input.clone());
                    config_changed = true;
                }
                if !bases::valid_column_key(self.base_sort_input.trim()) {
                    ui.colored_label(amber(), "valid key required");
                }
            });
        }
        ui.separator();
        let mut picked_tag = None;
        egui::ScrollArea::horizontal().id_salt("base_tags").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for tag in &tag_keys {
                    if ui
                        .add(
                            egui::Button::new(RichText::new(format!("#{tag}")).color(accent()))
                                .fill(crate::tag())
                                .small(),
                        )
                        .clicked()
                    {
                        picked_tag = Some(tag.clone());
                    }
                }
            });
        });
        if let Some(tag) = picked_tag {
            self.base_config.filter_tag = tag;
            config_changed = true;
        }
        if clear_filters_requested {
            bases::clear_filters(&mut self.base_config);
            config_changed = true;
        }

        let mut rows = bases::build(&self.index, &self.base_config);
        if rows.is_empty() {
            if bases::has_filters(&self.base_config) {
                ui.horizontal(|ui| {
                    ui.colored_label(muted(), "No notes match these filters.");
                    if ui.small_button("Clear filters").clicked() {
                        clear_filters_requested = true;
                    }
                });
                // Rebuild in the same frame so the rows return immediately.
                if clear_filters_requested {
                    bases::clear_filters(&mut self.base_config);
                    config_changed = true;
                    rows = bases::build(&self.index, &self.base_config);
                }
            } else {
                ui.colored_label(muted(), "No notes in this vault — Ctrl+N");
            }
        }
        ui.colored_label(muted(), format!("{} rows", rows.len()));
        let mut open_note = None;
        let group_key = self.base_config.group_by.trim().to_string();
        egui::ScrollArea::vertical().id_salt("base_rows").show(ui, |ui| {
            if self.base_cards {
                for row in &rows {
                    ui.group(|ui| {
                        let preview = preview::for_note(&self.index, &row.id, 8);
                        let response = ui.button(&row.title);
                        if response.clicked() {
                            open_note = Some(row.id.clone());
                        }
                        hover_preview(ui, response, preview);
                        ui.colored_label(muted(), row.id.as_str());
                        for key in ["created", "tags", "aliases"]
                            .into_iter()
                            .chain(
                                extra_columns
                                    .iter()
                                    .map(String::as_str)
                                    .filter(|key| *key != group_key),
                            )
                        {
                            if let Some(value) = row.values.get(key) {
                                if !value.is_empty() {
                                    ui.horizontal(|ui| {
                                        ui.colored_label(accent(), format!("{key}:"));
                                        ui.label(value);
                                    });
                                }
                            }
                        }
                        if !row.group.is_empty() {
                            ui.colored_label(muted(), format!("group: {}", row.group));
                        }
                    });
                }
            } else {
                let group_editable = bases::editable_columns(&self.base_config)
                    .iter()
                    .any(|column| column == &group_key);
                // §80: an explicit projection of editable cells makes keyboard
                // selection and Enter-to-edit unambiguous.
                let mut cell_rows: Vec<Vec<BaseCellRef>> = Vec::with_capacity(rows.len());
                let mut occurrences: std::collections::BTreeMap<(String, String), usize> =
                    std::collections::BTreeMap::new();
                for row in &rows {
                    let occurrence = {
                        let counter = occurrences
                            .entry((row.id.as_str().to_string(), row.group.clone()))
                            .or_insert(0);
                        let current = *counter;
                        *counter += 1;
                        current
                    };
                    let mut cells = Vec::new();
                    for key in ["created", "tags", "aliases"] {
                        cells.push(BaseCellRef {
                            id: row.id.clone(),
                            group: row.group.clone(),
                            occurrence,
                            column: key.to_string(),
                            key: key.to_string(),
                            value: row.values.get(key).cloned().unwrap_or_default(),
                        });
                    }
                    for key in extra_columns.iter().filter(|key| key.as_str() != group_key) {
                        cells.push(BaseCellRef {
                            id: row.id.clone(),
                            group: row.group.clone(),
                            occurrence,
                            column: key.clone(),
                            key: key.clone(),
                            value: row.values.get(key).cloned().unwrap_or_default(),
                        });
                    }
                    if group_editable {
                        cells.push(BaseCellRef {
                            id: row.id.clone(),
                            group: row.group.clone(),
                            occurrence,
                            column: "group".to_string(),
                            key: group_key.clone(),
                            value: row.group.clone(),
                        });
                    }
                    cell_rows.push(cells);
                }

                let signature = format!(
                    "{:?}",
                    (
                        rows.iter()
                            .map(|row| (row.id.as_str(), row.group.as_str()))
                            .collect::<Vec<_>>(),
                        extra_columns.clone(),
                        group_key.clone(),
                        group_editable,
                        self.base_config.filter_tag.clone(),
                        self.base_config.filter_text.clone(),
                        self.base_config.sort_by,
                        self.base_config.sort_desc,
                        self.base_config.sort_field.clone(),
                    )
                );
                if self.base_selection_key != signature {
                    self.base_selected = None;
                    self.base_selection_key = signature;
                }
                let columns = cell_rows.first().map_or(0, Vec::len);
                if self
                    .base_selected
                    .is_some_and(|(row, column)| row >= cell_rows.len() || column >= columns)
                {
                    self.base_selected = None;
                }

                let mut open_cell = false;
                // Only navigate when a cell is already selected and no editor
                // is active, so config TextEdits keep their arrow keys (§80).
                // Skip navigation while a config TextEdit owns focus, so its
                // caret keeps its arrow keys; otherwise a selected cell arms
                // the grid (§80 review). Enter is consumed below, preventing
                // egui from activating whichever button holds focus.
                let focused = ui.ctx().memory(|memory| memory.focused());
                let config_ids = [
                    egui::Id::unique("base_config_column"),
                    egui::Id::unique("base_config_tag"),
                    egui::Id::unique("base_config_group"),
                    egui::Id::unique("base_config_contains"),
                    egui::Id::unique("base_config_sort_field"),
                ];
                let config_focused = focused.is_some_and(|id| config_ids.contains(&id));
                if !self.modal_open()
                    && self.base_edit.is_none()
                    && self.base_selected.is_some()
                    && !config_focused
                {
                    let ctx = ui.ctx().clone();
                    let (up, down, left, right, enter) = ctx.input(|input| {
                        let plain = !input.modifiers.ctrl
                            && !input.modifiers.shift
                            && !input.modifiers.alt
                            && !input.modifiers.mac_cmd;
                        (
                            plain && input.key_pressed(Key::ArrowUp),
                            plain && input.key_pressed(Key::ArrowDown),
                            plain && input.key_pressed(Key::ArrowLeft),
                            plain && input.key_pressed(Key::ArrowRight),
                            plain && input.key_pressed(Key::Enter),
                        )
                    });
                    let direction = if up {
                        Some(bases::Move::Up)
                    } else if down {
                        Some(bases::Move::Down)
                    } else if left {
                        Some(bases::Move::Left)
                    } else if right {
                        Some(bases::Move::Right)
                    } else {
                        None
                    };
                    if let Some(direction) = direction {
                        self.base_selected = bases::move_selection(
                            cell_rows.len(),
                            columns,
                            self.base_selected,
                            direction,
                        );
                    } else if enter {
                        open_cell = true;
                    }
                    if direction.is_some() || enter {
                        ctx.input_mut(|input| {
                            input.events.retain(|event| {
                                !matches!(
                                    event,
                                    egui::Event::Key {
                                        key: Key::ArrowUp
                                            | Key::ArrowDown
                                            | Key::ArrowLeft
                                            | Key::ArrowRight
                                            | Key::Enter,
                                        pressed: true,
                                        modifiers,
                                        ..
                                    } if !modifiers.ctrl
                                        && !modifiers.shift
                                        && !modifiers.alt
                                        && !modifiers.mac_cmd
                                )
                            });
                        });
                    }
                }

                let mut cell_action = BaseCellAction::None;
                egui::ScrollArea::horizontal().id_salt("base_grid_scroll").show(ui, |ui| {
                egui::Grid::new("base_grid").striped(true).show(ui, |ui| {
                    ui.colored_label(muted(), "note");
                    ui.colored_label(muted(), "created");
                    ui.colored_label(muted(), "tags");
                    ui.colored_label(muted(), "aliases");
                    for key in extra_columns.iter().filter(|key| key.as_str() != group_key) {
                        ui.colored_label(muted(), key);
                    }
                    ui.colored_label(muted(), "group");
                    ui.end_row();
                    for (row_index, row) in rows.iter().enumerate() {
                        let title = if row.group.is_empty() {
                            row.title.clone()
                        } else {
                            format!("{} ({})", row.title, row.group)
                        };
                        let preview = preview::for_note(&self.index, &row.id, 8);
                        let response = ui.button(title);
                        if response.clicked() {
                            open_note = Some(row.id.clone());
                        }
                        hover_preview(ui, response, preview);
                        for (column_index, cell) in cell_rows[row_index].iter().enumerate() {
                            let index = (row_index, column_index);
                            let action = self.base_cell_ui(
                                ui,
                                cell,
                                index,
                                self.base_selected == Some(index),
                            );
                            if action != BaseCellAction::None {
                                cell_action = action;
                            }
                        }
                        ui.end_row();
                    }
                });
                });
                if open_cell {
                    if let Some((row, column)) = self.base_selected {
                        if let Some(cell) = cell_rows.get(row).and_then(|cells| cells.get(column)) {
                            self.base_edit = Some(BaseEdit {
                                id: cell.id.clone(),
                                group: cell.group.clone(),
                                occurrence: cell.occurrence,
                                column: cell.column.clone(),
                                key: cell.key.clone(),
                                value: cell.value.clone(),
                            });
                        }
                    }
                }
                match cell_action {
                    BaseCellAction::Commit => {
                        if let Some(edit) = self.base_edit.take() {
                            self.commit_base_edit(edit);
                        }
                        // The editor owned focus; require a fresh cell click
                        // before keyboard nav resumes (§80 review).
                        self.base_selected = None;
                    }
                    BaseCellAction::Cancel => {
                        self.base_edit = None;
                    }
                    BaseCellAction::None => {}
                }
            }
        });
        if config_changed {
            self.save_base_config();
        }
        if let Some(id) = open_note {
            self.open_note(&id);
        }
    }

    /// Canvas tab: fit all cards into the panel, drag to move, click to open.
    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Reset layout").clicked() {
                self.canvas = canvas::initial(&self.index);
                let path = self.canvas_path.clone();
                if let Err(error) = canvas::save(&path, &self.canvas) {
                    self.error = Some(error.to_string());
                }
                self.status = "Canvas layout reset".to_string();
            }
            ui.colored_label(muted(), "drag cards · click to open");
        });
        ui.separator();
        if self.canvas.nodes.is_empty() {
            ui.label("No notes in this vault.");
            self.canvas_selected = None;
            self.canvas_selection_key.clear();
            return;
        }

        // §76: selection belongs to one node set, never to a stale layout.
        let selection_signature = canvas::selection_key(&self.canvas);
        if self.canvas_selection_key != selection_signature {
            self.canvas_selected = None;
            self.canvas_selection_key = selection_signature;
        }
        let ids: Vec<NoteId> = self.canvas.nodes.keys().cloned().collect();
        if self
            .canvas_selected
            .as_ref()
            .is_some_and(|id| !ids.contains(id))
        {
            self.canvas_selected = None;
        }

        let mut open_selected = None;
        if !self.modal_open() {
            let ctx = ui.ctx().clone();
            let (previous, next, enter) = ctx.input(|input| {
                (
                    input.key_pressed(Key::ArrowLeft) || input.key_pressed(Key::ArrowUp),
                    input.key_pressed(Key::ArrowRight) || input.key_pressed(Key::ArrowDown),
                    input.key_pressed(Key::Enter),
                )
            });
            if (previous || next) && !ids.is_empty() {
                let current = self
                    .canvas_selected
                    .as_ref()
                    .and_then(|id| ids.iter().position(|item| item == id));
                let index = match current {
                    Some(position) if previous => (position + ids.len() - 1) % ids.len(),
                    Some(position) => (position + 1) % ids.len(),
                    None if previous => ids.len() - 1,
                    None => 0,
                };
                self.canvas_selected = Some(ids[index].clone());
            }
            if enter {
                open_selected = self.canvas_selected.clone();
            }
            if previous || next || enter {
                ctx.input_mut(|input| {
                    input.events.retain(|event| {
                        !matches!(
                            event,
                            egui::Event::Key {
                                key: Key::ArrowLeft
                                    | Key::ArrowUp
                                    | Key::ArrowRight
                                    | Key::ArrowDown
                                    | Key::Enter,
                                pressed: true,
                                ..
                            }
                        )
                    });
                });
            }
        }

        let size = ui.available_size_before_wrap() - Vec2::splat(8.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let painter = ui.painter();
        let min_x = self.canvas.nodes.values().map(|pos| pos.x).fold(f64::INFINITY, f64::min) - 130.0;
        let max_x = self.canvas.nodes.values().map(|pos| pos.x).fold(f64::NEG_INFINITY, f64::max) + 130.0;
        let min_y = self.canvas.nodes.values().map(|pos| pos.y).fold(f64::INFINITY, f64::min) - 50.0;
        let max_y = self.canvas.nodes.values().map(|pos| pos.y).fold(f64::NEG_INFINITY, f64::max) + 50.0;
        let span_x = (max_x - min_x).max(1.0) as f32;
        let span_y = (max_y - min_y).max(1.0) as f32;
        let scale = (rect.width() / span_x)
            .min(rect.height() / span_y)
            .min(1.0)
            .max(0.15);
        let origin_x = rect.center().x - (min_x as f32 + span_x / 2.0) * scale;
        let origin_y = rect.center().y - (min_y as f32 + span_y / 2.0) * scale;
        let to_screen = |pos: canvas::Pos| {
            egui::pos2(origin_x + pos.x as f32 * scale, origin_y + pos.y as f32 * scale)
        };
        let card_size = egui::vec2(200.0 * scale, 52.0 * scale);

        for (from, to) in canvas::edges(&self.index) {
            if let (Some(a), Some(b)) = (
                self.canvas.nodes.get(&from),
                self.canvas.nodes.get(&to),
            ) {
                painter.line_segment(
                    [to_screen(*a), to_screen(*b)],
                    Stroke::new(1.5, line()),
                );
            }
        }

        for id in &ids {
            if let Some(pos) = self.canvas.nodes.get(id) {
                let center = to_screen(*pos);
                let card = egui::Rect::from_center_size(center, card_size);
                let selected = self.canvas_selected.as_ref() == Some(id);
                let stroke = if selected {
                    Stroke::new(2.0, accent())
                } else {
                    Stroke::new(1.0, line())
                };
                painter.rect_filled(card, 6.0, surface());
                painter.rect_stroke(card, 6.0, stroke, egui::StrokeKind::Outside);
                let title = self
                    .index
                    .notes
                    .get(id)
                    .map(|note| note.title.clone())
                    .unwrap_or_else(|| id.stem().to_string());
                painter.text(
                    card.center(),
                    egui::Align2::CENTER_CENTER,
                    title,
                    egui::FontId::proportional(13.0 * scale),
                    text(),
                );
            }
        }

        let pointer = response.hover_pos();
        let mut click_open = None;
        let mut start_drag = None;
        if let Some(at) = pointer {
            let hit = ids.iter().rev().find_map(|id| {
                self.canvas.nodes.get(id).and_then(|pos| {
                    let card = egui::Rect::from_center_size(to_screen(*pos), card_size);
                    card.contains(at).then(|| (id.clone(), at - card.center()))
                })
            });
            if response.drag_started() {
                start_drag = hit.clone();
            }
            if response.clicked() {
                click_open = hit.map(|(id, _)| id);
            }
        }
        if let Some((id, offset)) = start_drag {
            self.canvas_drag = Some((id, offset));
        }
        if let (Some((id, offset)), Some(at)) = (&self.canvas_drag, pointer) {
            if response.dragged() {
                let center = at - *offset;
                let pos = canvas::Pos {
                    x: ((center.x - origin_x) / scale) as f64,
                    y: ((center.y - origin_y) / scale) as f64,
                };
                if let Some(node) = self.canvas.nodes.get_mut(id) {
                    *node = pos;
                }
            }
        }
        if response.drag_stopped() {
            let path = self.canvas_path.clone();
            if let Err(error) = canvas::save(&path, &self.canvas) {
                self.error = Some(error.to_string());
            } else {
                self.status = "Canvas positions saved".to_string();
            }
            self.canvas_drag = None;
        }
        if let Some(id) = click_open.or(open_selected) {
            self.open_note(&id);
        }
    }

    fn draw_graph(&mut self, ui: &mut egui::Ui) {
        let layout = graph::build(&self.index);
        if layout.truncated {
            ui.colored_label(amber(), "Showing 500 highest-degree notes of a larger vault");
        }

        // §77: selection belongs to one visible node set, never a stale id.
        let signature = graph::selection_key(&layout);
        if self.graph_selection_key != signature {
            self.graph_selected = None;
            self.graph_selection_key = signature;
        }
        let ids: Vec<NoteId> = layout.nodes.iter().map(|node| node.id.clone()).collect();
        if self
            .graph_selected
            .as_ref()
            .is_some_and(|id| !ids.contains(id))
        {
            self.graph_selected = None;
        }

        let mut open_selected = None;
        if !self.modal_open() {
            let ctx = ui.ctx().clone();
            let (previous, next, enter) = ctx.input(|input| {
                (
                    input.key_pressed(Key::ArrowLeft) || input.key_pressed(Key::ArrowUp),
                    input.key_pressed(Key::ArrowRight) || input.key_pressed(Key::ArrowDown),
                    input.key_pressed(Key::Enter),
                )
            });
            if (previous || next) && !ids.is_empty() {
                let current = self
                    .graph_selected
                    .as_ref()
                    .and_then(|id| ids.iter().position(|item| item == id));
                let index = match current {
                    Some(position) if previous => (position + ids.len() - 1) % ids.len(),
                    Some(position) => (position + 1) % ids.len(),
                    None if previous => ids.len() - 1,
                    None => 0,
                };
                self.graph_selected = Some(ids[index].clone());
            }
            if enter {
                open_selected = self.graph_selected.clone();
            }
            if previous || next || enter {
                ctx.input_mut(|input| {
                    input.events.retain(|event| {
                        !matches!(
                            event,
                            egui::Event::Key {
                                key: Key::ArrowLeft
                                    | Key::ArrowUp
                                    | Key::ArrowRight
                                    | Key::ArrowDown
                                    | Key::Enter,
                                pressed: true,
                                ..
                            }
                        )
                    });
                });
            }
        }

        let size = ui.available_size_before_wrap() - Vec2::splat(12.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let painter = ui.painter();
        let center = rect.center();
        let scale_x = rect.width() * 0.45;
        let scale_y = rect.height() * 0.45;
        let point = |node: &graph::GraphNode| {
            egui::pos2(
                center.x + node.x as f32 * scale_x,
                center.y + node.y as f32 * scale_y,
            )
        };
        let positions: std::collections::BTreeMap<&NoteId, egui::Pos2> = layout
            .nodes
            .iter()
            .map(|node| (&node.id, point(node)))
            .collect();
        for edge in &layout.edges {
            if let (Some(from), Some(to)) = (
                positions.get(&edge.source),
                positions.get(&edge.target),
            ) {
                painter.line_segment([*from, *to], Stroke::new(1.0, line()));
            }
        }
        for node in &layout.nodes {
            let at = point(node);
            let selected = self.graph_selected.as_ref() == Some(&node.id);
            let radius = if selected { 6.0 } else { 4.0 };
            if selected {
                painter.circle_stroke(at, radius + 2.0, Stroke::new(1.5, accent()));
            }
            painter.circle_filled(at, radius, accent());
            painter.text(
                at + Vec2::new(6.0, 0.0),
                egui::Align2::LEFT_CENTER,
                node.id.stem(),
                egui::FontId::proportional(11.0),
                text(),
            );
        }
        // Click a node to open it, same as Canvas (§76).
        if response.clicked() {
            if let Some(at) = response.hover_pos() {
                let hit = layout
                    .nodes
                    .iter()
                    .find(|node| {
                        let point = point(node);
                        (point - at).length() <= 12.0
                    })
                    .map(|node| node.id.clone());
                if let Some(id) = hit {
                    open_selected = Some(id);
                }
            }
        }
        if let Some(id) = open_selected {
            self.open_note(&id);
        }
    }

    /// Returns the note to open and, separately, a broken-link target to
    /// create. The caller executes the command after the panel closes (§49).
    fn draw_health(
        &mut self,
        ui: &mut egui::Ui,
    ) -> (Option<(NoteId, Option<usize>)>, Option<NoteId>) {
        let issues = health::check(&self.index);

        // §72: the selection belongs to one report snapshot, never to a stale
        // list after links were fixed or notes were removed.
        let key = health::selection_key(&issues);
        if self.health_selection_key != key {
            self.health_selected = 0;
            self.health_selection_key = key;
        }
        self.health_selected = self.health_selected.min(issues.len().saturating_sub(1));

        let mut open_selected = false;
        let mut scroll_to_selected = false;
        if !self.modal_open() {
            let ctx = ui.ctx().clone();
            let (up, down, enter) = ctx.input(|input| {
                (
                    input.key_pressed(Key::ArrowUp),
                    input.key_pressed(Key::ArrowDown),
                    input.key_pressed(Key::Enter),
                )
            });
            if (up || down) && !issues.is_empty() {
                if up {
                    self.health_selected = self.health_selected.saturating_sub(1);
                } else if self.health_selected + 1 < issues.len() {
                    self.health_selected += 1;
                }
                scroll_to_selected = true;
            }
            if enter && !issues.is_empty() {
                open_selected = true;
            }
            if up || down || enter {
                ctx.input_mut(|input| {
                    input.events.retain(|event| {
                        !matches!(
                            event,
                            egui::Event::Key {
                                key: Key::ArrowUp | Key::ArrowDown | Key::Enter,
                                pressed: true,
                                ..
                            }
                        )
                    });
                });
            }
        }

        let mut chosen = None;
        let mut create = None;
        // One `create` per missing target, even if many notes link to it.
        let mut seen: Vec<NoteId> = Vec::new();
        egui::ScrollArea::vertical().id_salt("health_issues").show(ui, |ui| {
            if issues.is_empty() {
                ui.label("Vault health: no issues found.");
            } else {
                for (index, issue) in issues.iter().enumerate() {
                    let color = match issue.severity {
                        Severity::Error => rust(),
                        Severity::Warning => amber(),
                        Severity::Info => sage(),
                    };
                    ui.horizontal(|ui| {
                        ui.colored_label(color, format!("{:?}", issue.severity));
                        let where_text = issue
                            .line
                            .map(|line| format!("line {line}"))
                            .unwrap_or_else(|| "note".to_string());
                        // Orphan rows share one message, so include the note
                        // to make the keyboard target visible (§72 review).
                        let note_label = issue
                            .note
                            .as_ref()
                            .map(NoteId::as_str)
                            .unwrap_or("-");
                        let response = ui.selectable_label(
                            index == self.health_selected,
                            format!("{} · {} · {}", issue.message, where_text, note_label),
                        );
                        if response.clicked() {
                            self.health_selected = index;
                        }
                        if scroll_to_selected && index == self.health_selected {
                            response.scroll_to_me(Some(egui::Align::Center));
                        }
                        if let Some(note) = &issue.note {
                            if ui.small_button("open").clicked() {
                                self.health_selected = index;
                                chosen = Some((note.clone(), issue.line));
                            }
                        }
                        // §49: a broken-link issue carries the missing target.
                        if let Some(target) = &issue.target {
                            let id = NoteId::new(target.clone());
                            if !self.index.notes.contains_key(&id) && !seen.contains(&id) {
                                if ui.small_button("create").clicked() {
                                    self.health_selected = index;
                                    create = Some(id.clone());
                                }
                                seen.push(id);
                            }
                        }
                    });
                }
            }
        });
        if open_selected {
            if let Some(issue) = issues.get(self.health_selected) {
                if let Some(note) = &issue.note {
                    chosen = Some((note.clone(), issue.line));
                }
            }
        }
        (chosen, create)
    }

    fn draw_search(&mut self, ui: &mut egui::Ui) -> Option<NoteId> {
        let mut hint = "";
        ui.horizontal(|ui| {
            for (mode, label, mode_hint) in [
                (
                    SearchMode::Keyword,
                    "Keyword",
                    "lexical vectors · local, no model, no network",
                ),
                (
                    SearchMode::Semantic,
                    "Semantic",
                    "lexical vectors · local, no model, no network",
                ),
                (
                    SearchMode::Hybrid,
                    "Hybrid",
                    "lexical vectors · local, no model, no network",
                ),
                (
                    SearchMode::Query,
                    "Query",
                    "boolean: AND OR NOT · field:value · [field:value]",
                ),
            ] {
                if ui
                    .selectable_label(self.search_mode == mode, label)
                    .clicked()
                {
                    self.search_mode = mode;
                }
                if self.search_mode == mode {
                    hint = mode_hint;
                }
            }
        });
        ui.colored_label(muted(), hint);
        if self.semantic_stale {
            ui.colored_label(amber(), "index stale — press Vectors to rebuild");
        }
        ui.separator();

        let query = self.search_query.clone();
        let keyword = search::search(&self.index, &query);
        let mut rows: Vec<(NoteId, String, String)> = Vec::new();
        match self.search_mode {
            SearchMode::Keyword => {
                for result in &keyword {
                    rows.push((
                        result.id.clone(),
                        result.title.clone(),
                        format!("rank {}", result.rank),
                    ));
                }
            }
            SearchMode::Semantic => {
                if let Some(index) = &self.semantic_index {
                    let vector = hash_embed(&query, index.dims);
                    for hit in search_semantic(index, &vector, 20) {
                        let title = self
                            .index
                            .notes
                            .get(&hit.id)
                            .map(|note| note.title.clone())
                            .unwrap_or_else(|| hit.id.stem().to_string());
                        rows.push((
                            hit.id,
                            title,
                            format!("line {} · {:.2}", hit.line, hit.score),
                        ));
                    }
                } else {
                    ui.colored_label(amber(), "no vector index — press Vectors");
                }
            }
            SearchMode::Hybrid => {
                let mut hits = Vec::new();
                if let Some(index) = &self.semantic_index {
                    let vector = hash_embed(&query, index.dims);
                    hits = search_semantic(index, &vector, 20);
                }
                for ranked in semantic::blend(&keyword, &hits, 0.5) {
                    let title = self
                        .index
                        .notes
                        .get(&ranked.id)
                        .map(|note| note.title.clone())
                        .unwrap_or_else(|| ranked.id.stem().to_string());
                    let source = match ranked.source {
                        RankSource::Keyword => "keyword",
                        RankSource::Semantic => "vector",
                        RankSource::Both => "both",
                    };
                    rows.push((
                        ranked.id,
                        title,
                        format!("{source} · {:.2}", ranked.score),
                    ));
                }
            }
            SearchMode::Query => {
                match search::validate(&query) {
                    Ok(()) => {
                        for result in &search::search_advanced(&self.index, &query) {
                            rows.push((result.id.clone(), result.title.clone(), "query".to_string()));
                        }
                    }
                    Err(error) => {
                        ui.colored_label(amber(), format!("Malformed query: {error}"));
                    }
                }
            }
        }

        // §64: selection belongs to one query/mode pair and never points at a
        // row that no longer exists.
        let signature = (query.clone(), self.search_mode);
        if self.search_selection_key != signature {
            self.search_selected = 0;
            self.search_selection_key = signature;
        }
        self.search_selected = self.search_selected.min(rows.len().saturating_sub(1));

        let mut open_selected = false;
        let mut scroll_to_selected = false;
        if !self.modal_open() {
            let ctx = ui.ctx().clone();
            let (up, down, enter) = ctx.input(|input| {
                (
                    input.key_pressed(Key::ArrowUp),
                    input.key_pressed(Key::ArrowDown),
                    input.key_pressed(Key::Enter),
                )
            });
            if (up || down) && !rows.is_empty() {
                if up {
                    self.search_selected = self.search_selected.saturating_sub(1);
                } else if self.search_selected + 1 < rows.len() {
                    self.search_selected += 1;
                }
                scroll_to_selected = true;
            }
            if enter && !rows.is_empty() {
                open_selected = true;
            }
            if up || down || enter {
                ctx.input_mut(|input| {
                    input.events.retain(|event| {
                        !matches!(
                            event,
                            egui::Event::Key {
                                key: Key::ArrowUp | Key::ArrowDown | Key::Enter,
                                pressed: true,
                                ..
                            }
                        )
                    });
                });
            }
        }

        let mut chosen = None;
        egui::ScrollArea::vertical().id_salt("search_results").show(ui, |ui| {
            if query.trim().is_empty() && self.search_mode == SearchMode::Keyword {
                ui.colored_label(muted(), "Type to search notes");
            }
            ui.label(format!("{} results", rows.len()));
            for (index, (id, title, meta)) in rows.iter().enumerate() {
                let response = ui.selectable_label(
                    index == self.search_selected,
                    format!("{title} — {id}"),
                );
                if response.clicked() {
                    self.search_selected = index;
                    chosen = Some(id.clone());
                }
                if scroll_to_selected && index == self.search_selected {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                ui.colored_label(muted(), format!("  {meta}"));
            }
        });
        if open_selected {
            chosen = rows.get(self.search_selected).map(|row| row.0.clone());
        }
        chosen
    }

    /// Resolve the `[[target]]` containing a byte offset produced by the
    /// galley's hit test. Used for Ctrl+Click navigation in the editor.
    fn wikilink_at(content: &str, char_index: usize) -> Option<NoteId> {
        let byte_index = content
            .char_indices()
            .nth(char_index)
            .map(|(offset, _)| offset)
            .unwrap_or(content.len());
        let before = content.get(..byte_index)?;
        let after = content.get(byte_index..)?;
        let start = before.rfind("[[")?;
        let end = byte_index + after.find("]]")?;
        if end <= start + 2 {
            return None;
        }
        let inside = content.get(start + 2..end)?;
        let target = inside.split('|').next()?.trim();
        (!target.is_empty()).then(|| NoteId::new(target))
    }

    /// Recursive explorer row renderer. Folder paths are the collapse keys;
    /// everything is expanded by default (`ARCHITECTURE.md` §17).
    /// Draw the tree and collect visible note rows for keyboard navigation:
    /// `(note id, widget id, has focus, rect)` (§68).
    fn draw_tree(
        &mut self,
        ui: &mut egui::Ui,
        nodes: &[TreeNode],
        depth: usize,
        note_rows: &mut Vec<(NoteId, egui::Id, bool, egui::Rect)>,
    ) {
        for node in nodes {
            ui.horizontal(|ui| {
                ui.add_space((depth * 13) as f32);
                let node_id = node.id_key();
                ui.push_id(&node_id, |ui| match &node.kind {
                    NodeKind::Folder => {
                        let collapsed = self.collapsed.contains(&node.path);
                        let chevron = if collapsed { ">" } else { "v" };
                        if ui
                            .selectable_label(false, format!("{} {}/", chevron, node.name))
                            .clicked()
                        {
                            if collapsed {
                                self.collapsed.remove(&node.path);
                            } else {
                                self.collapsed.insert(node.path.clone());
                            }
                        }
                    }
                    NodeKind::Note(id) => {
                        let active = self
                            .tabs
                            .get(self.active)
                            .is_some_and(|tab| tab.kind == TabKind::Note && tab.id == id.as_str());
                        let preview = preview::for_note(&self.index, id, 8);
                        let title = self
                            .index
                            .notes
                            .get(id)
                            .map(|note| note.title.clone())
                            .unwrap_or_else(|| node.name.clone());
                        let mut clicked = false;
                        ui.vertical(|ui| {
                            let response = ui.selectable_label(active, &title);
                            clicked = response.clicked();
                            if clicked {
                                // Keep focus on the row so ArrowUp/Down can
                                // continue from the clicked note (§68).
                                ui.memory_mut(|memory| memory.request_focus(response.id));
                            }
                            note_rows.push((
                                id.clone(),
                                response.id,
                                response.has_focus(),
                                response.rect,
                            ));
                            hover_preview(ui, response, preview);
                            ui.colored_label(muted(), RichText::new(id.as_str()).size(12.0));
                        });
                        if clicked {
                            self.open_note(id);
                        }
                    }
                });
            });
            if matches!(node.kind, NodeKind::Folder) && !self.collapsed.contains(&node.path) {
                self.draw_tree(ui, &node.children, depth + 1, note_rows);
            }
        }
    }

    /// Replace the typed `[[partial` (and optional `|alias`) with a complete
    /// wikilink, then leave the caret after the closing brackets.
    fn insert_link(&mut self, ctx: &egui::Context, target: &str) {
        let Some(popup) = self.link_popup.take() else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let end = popup.cursor.min(tab.content.chars().count());
        let start = popup.start.min(end);
        let insertion: Vec<char> = format!("[[{target}]]").chars().collect();
        let mut chars: Vec<char> = tab.content.chars().collect();
        chars.splice(start..end, insertion.iter().copied());
        tab.content = chars.into_iter().collect();
        tab.dirty = true;
        tab.last_edit = Some(Instant::now());

        let new_cursor = start + insertion.len();
        let editor_id = egui::Id::unique(("nexora_editor", tab.id.clone()));
        set_editor_cursor(ctx, editor_id, new_cursor);
    }
}

impl eframe::App for NexoraApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.autosave();
        self.poll_disk();
        // Keep polling alive when the window is idle and no input arrives.
        ctx.request_repaint_after(Duration::from_secs(1));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // A Base cell editor belongs to the Base tab that is currently shown.
        // Clearing here covers tab clicks, closes and workspace applies too.
        if !self
            .tabs
            .get(self.active)
            .is_some_and(|tab| tab.kind == TabKind::Base)
        {
            self.base_edit = None;
        }
        if self.theme_dirty {
            apply_style(&ctx, self.theme);
            self.theme_dirty = false;
        }
        // 1. Input -> actions (collected first so no UI borrow is held).
        let keys = Self::read_keyboard(&ctx);
        let mut popup_keys: Option<(bool, bool, bool, bool, bool)> = None;
        // §83: a modal owns the keyboard. Escape is handled by the modal
        // itself; every global action below is suppressed while one is open.
        let global_keys_allowed = !self.modal_open();
        if global_keys_allowed && keys.quick_open {
            self.execute(CommandId::QuickOpen);
        }
        if global_keys_allowed && keys.goto_heading {
            self.execute(CommandId::GoToHeading);
        }
        if global_keys_allowed && keys.rename {
            self.execute(CommandId::RenameNote);
        }
        if global_keys_allowed && keys.toggle_view {
            if let Some(tab) = self.tabs.get_mut(self.active) {
                if tab.kind == TabKind::Note {
                    tab.view = match tab.view {
                        ViewMode::Source => ViewMode::Reading,
                        ViewMode::Reading => ViewMode::Source,
                    };
                }
            }
        }
        if global_keys_allowed && keys.palette {
            self.palette_open = true;
            self.palette_query.clear();
            self.palette_selected = 0;
            self.palette_selection_key.clear();
        }
        if global_keys_allowed && keys.new_note {
            self.execute(CommandId::NewNote);
        }
        if global_keys_allowed && keys.capture {
            self.execute(CommandId::QuickCapture);
        }
        if global_keys_allowed && keys.search {
            self.execute(CommandId::Search);
        }
        if global_keys_allowed && keys.close_tab {
            self.execute(CommandId::CloseTab);
        }
        // §73: Ctrl+Tab / Ctrl+Shift+Tab cycle tabs, but only outside modals.
        if (keys.next_tab || keys.prev_tab) && !self.modal_open() {
            if let Some(next) = tab_nav::cycle(self.active, self.tabs.len(), keys.prev_tab) {
                self.active = next;
            }
            ctx.input_mut(|input| {
                input.events.retain(|event| {
                    !matches!(
                        event,
                        egui::Event::Key {
                            key: Key::Tab,
                            pressed: true,
                            modifiers,
                            ..
                        } if modifiers.ctrl
                    )
                });
            });
        }

        // 2. Autosave and external-change polling run in App::logic.
        let conflict = self.conflict.clone();
        let error = self.error.clone();

        // 3. Toolbar.
        egui::Panel::top("toolbar")
            .exact_size(40.0)
            .frame(egui::Frame::new().fill(panel()).inner_margin(Margin::symmetric(14, 6)))
            .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(accent(), "N E X O R A");
                ui.add_space(10.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text("Search notes...")
                        .desired_width(170.0),
                );
                if ui.button("Search").clicked() {
                    self.execute(CommandId::Search);
                }
                if ui.button("Today").clicked() {
                    self.execute(CommandId::OpenToday);
                }
                if ui.button("Health").clicked() {
                    self.execute(CommandId::HealthCheck);
                }
                if ui.button("Graph").clicked() {
                    self.execute(CommandId::OpenGraph);
                }
                if ui.button("Canvas").clicked() {
                    self.execute(CommandId::OpenCanvas);
                }
                if ui.button("Base").clicked() {
                    self.execute(CommandId::OpenBase);
                }
                if ui.button("Vectors").clicked() {
                    self.execute(CommandId::BuildSemanticIndex);
                }
                if ui.button("Capture").clicked() {
                    self.execute(CommandId::QuickCapture);
                }
                if ui.button("Ctrl+P").clicked() {
                    self.palette_open = true;
                    self.palette_query.clear();
                    self.palette_selected = 0;
                    self.palette_selection_key.clear();
                }
            });
            });

        // 4a. Functional icon rail (§85): 48 px, linear icons, no emoji.
        let active_kind = self.tabs.get(self.active).map(|tab| tab.kind);
        egui::Panel::left("icon_rail")
            .default_size(48.0)
            .min_size(48.0)
            .max_size(48.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(bg()).inner_margin(Margin::symmetric(8, 12)))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(2.0);
                    if rail_button(ui, RailIcon::Today, "Today", false) {
                        self.execute(CommandId::OpenToday);
                    }
                    if rail_button(ui, RailIcon::Health, "Health", active_kind == Some(TabKind::Health)) {
                        self.execute(CommandId::HealthCheck);
                    }
                    if rail_button(ui, RailIcon::Graph, "Graph", active_kind == Some(TabKind::Graph)) {
                        self.execute(CommandId::OpenGraph);
                    }
                    if rail_button(ui, RailIcon::Canvas, "Canvas", active_kind == Some(TabKind::Canvas)) {
                        self.execute(CommandId::OpenCanvas);
                    }
                    if rail_button(ui, RailIcon::Base, "Base", active_kind == Some(TabKind::Base)) {
                        self.execute(CommandId::OpenBase);
                    }
                    if rail_button(ui, RailIcon::Vectors, "Vectors", false) {
                        self.execute(CommandId::BuildSemanticIndex);
                    }
                    if rail_button(ui, RailIcon::Capture, "Capture", false) {
                        self.execute(CommandId::QuickCapture);
                    }
                });
            });

        // 4b. Left: hierarchical explorer with optional tag filter.
        let tree_ids: Vec<NoteId> = self
            .index
            .notes
            .iter()
            .filter(|(_, note)| match &self.tag_filter {
                None => true,
                Some(tag) => note.content.contains(&format!("#{tag}")),
            })
            .map(|(id, _)| id.clone())
            .collect();
        let tree_empty = tree_ids.is_empty();
        let tree = build_tree(tree_ids);
        let root_name = self
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("vault")
            .to_string();
        let all_tags: Vec<String> = self.index.tags.keys().cloned().collect();
        let bookmark_rows: Vec<NoteId> = self.bookmarks.ids.clone();
        let mut tree_note_rows: Vec<(NoteId, egui::Id, bool, egui::Rect)> = Vec::new();
        egui::Panel::left("notes")
            .default_size(240.0)
            .frame(egui::Frame::new().fill(panel()).inner_margin(12.0))
            .show(ui, |ui| {
            ui.colored_label(muted(), "EXPLORER");
            if !bookmark_rows.is_empty() {
                ui.separator();
                ui.colored_label(muted(), "BOOKMARKS");
                for id in &bookmark_rows {
                    self.note_row(ui, id.as_str().to_string(), id);
                }
                ui.separator();
            }
            if ui.button("All notes").clicked() {
                self.tag_filter = None;
            }
            ui.horizontal_wrapped(|ui| {
                for tag in &all_tags {
                    if ui
                        .add(
                            egui::Button::new(RichText::new(format!("#{tag}")).color(accent()))
                                .fill(crate::tag())
                                .small(),
                        )
                        .clicked()
                    {
                        self.tag_filter = Some(tag.clone());
                    }
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().id_salt("notes_list").show(ui, |ui| {
                if tree_empty {
                    if self.tag_filter.is_some() {
                        ui.colored_label(muted(), "No notes with this tag — All notes to clear");
                    } else {
                        ui.colored_label(muted(), "No notes yet — Ctrl+N");
                    }
                }
                const ROOT_KEY: &str = "\u{1}root";
                let root_collapsed = self.collapsed.contains(ROOT_KEY);
                let chevron = if root_collapsed { ">" } else { "v" };
                if ui
                    .selectable_label(false, format!("{} {}/", chevron, root_name))
                    .clicked()
                {
                    if root_collapsed {
                        self.collapsed.remove(ROOT_KEY);
                    } else {
                        self.collapsed.insert(ROOT_KEY.to_string());
                    }
                }
                if !root_collapsed {
                    self.draw_tree(ui, &tree, 1, &mut tree_note_rows);
                }
            });
        });
        // §68: move focus only while a tree note is focused and no modal owns
        // the keyboard. The widget id from the drawn Response is stable under
        // the existing `push_id(&node.path)`.
        if !self.modal_open() {
            let focused = tree_note_rows.iter().position(|row| row.2);
            let (up, down) = ui.ctx().input(|input| {
                (
                    input.key_pressed(Key::ArrowUp),
                    input.key_pressed(Key::ArrowDown),
                )
            });
            if let Some(index) = focused {
                // `vertical_arrows: true` marks the arrows as consumed by this
                // widget, so egui does not apply its own spatial focus pass
                // (§68). Focus then moves only across visible note rows here.
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        tree_note_rows[index].1,
                        egui::EventFilter {
                            vertical_arrows: true,
                            ..Default::default()
                        },
                    );
                });
                if up || down {
                    let next = if up {
                        index.saturating_sub(1)
                    } else {
                        (index + 1).min(tree_note_rows.len() - 1)
                    };
                    if next != index {
                        ui.memory_mut(|memory| memory.request_focus(tree_note_rows[next].1));
                        ui.scroll_to_rect(tree_note_rows[next].3, None);
                    }
                    ui.ctx().input_mut(|input| {
                        input.events.retain(|event| {
                            !matches!(
                                event,
                                egui::Event::Key {
                                    key: Key::ArrowUp | Key::ArrowDown,
                                    ..
                                }
                            )
                        });
                    });
                    // The focus-lock filter needs a frame of history; clear
                    // egui's pending cardinal move so a fast second arrow
                    // cannot double-step before the filter is installed.
                    ui.memory_mut(|memory| {
                        memory.move_focus(egui::FocusDirection::None);
                    });
                }
            }
        }

        // 5. Right: links, backlinks, tags, version history.
        let current = self.current_note_id();
        let outgoing_rows: Vec<outgoing::Outgoing> = current
            .as_ref()
            .map(|id| outgoing::for_note(&self.index, id))
            .unwrap_or_default();
        let backlinks: Vec<NoteId> = current
            .as_ref()
            .and_then(|id| self.index.backlinks.get(id))
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default();
        let note_tags: Vec<String> = current
            .as_ref()
            .map(|id| {
                self.index
                    .tags
                    .iter()
                    .filter(|(_, notes)| notes.contains(id))
                    .map(|(tag, _)| tag.clone())
                    .collect()
            })
            .unwrap_or_default();
        let snapshots: Vec<String> = current
            .as_ref()
            .and_then(|id| save::list_snapshots(&self.root, id).ok())
            .unwrap_or_default();
        // Cache by target + vector fingerprint: cosine scoring is O(n²) and
        // must not run on every GUI frame (ARCHITECTURE.md §26).
        let related: Vec<Related> = match (&current, &self.semantic_index) {
            (Some(id), Some(semantic)) => {
                let cached = self
                    .related_cache
                    .as_ref()
                    .filter(|(target, fingerprint, _)| target == id && *fingerprint == semantic.fingerprint)
                    .map(|(_, _, rows)| rows.clone());
                match cached {
                    Some(rows) => rows,
                    None => {
                        let rows = related::suggest(&self.index, semantic, id, 5);
                        self.related_cache = Some((id.clone(), semantic.fingerprint, rows.clone()));
                        rows
                    }
                }
            }
            _ => Vec::new(),
        };
        let unlinked = current
            .as_ref()
            .map(|id| mentions::find(&self.index, id))
            .unwrap_or_default();
        egui::Panel::right("context")
            .default_size(240.0)
            .frame(egui::Frame::new().fill(panel()).inner_margin(12.0))
            .show(ui, |ui| {
            ui.colored_label(muted(), "CONTEXT");
            ui.separator();
            ui.colored_label(muted(), "RELATED");
            if self.semantic_index.is_none() {
                ui.colored_label(amber(), "build vectors to enable");
            } else if self.semantic_stale {
                ui.colored_label(amber(), "vectors stale — rebuild");
            } else if related.is_empty() {
                ui.colored_label(muted(), "no close notes yet");
            }
            for row in &related {
                self.note_row(ui, row.id.as_str().to_string(), &row.id);
                ui.colored_label(
                    muted(),
                    format!(
                        "sim {:.2} · tag {:.2} · graph {:.2}",
                        row.factors.semantic, row.factors.tag, row.factors.graph
                    ),
                );
            }
            ui.separator();
            ui.colored_label(muted(), "OUTGOING LINKS");
            if outgoing_rows.is_empty() {
                ui.colored_label(muted(), "none yet");
            }
            for row in &outgoing_rows {
                let anchor = row
                    .anchor
                    .as_ref()
                    .map(|value| format!("#{value}"))
                    .unwrap_or_default();
                let label = if row.resolved {
                    format!(
                        "{}{anchor}",
                        row.alias
                            .clone()
                            .unwrap_or_else(|| row.target.as_str().to_string())
                    )
                } else {
                    format!("{}{anchor} (missing)", row.target)
                };
                let text = format!("{label} · line {}", row.line);
                if row.resolved {
                    if ui.button(text).clicked() {
                        self.open_note(&row.target);
                        if let Some(value) = row.anchor.as_ref() {
                            self.jump_to_anchor(&row.target, value);
                        }
                    }
                } else if ui.button(RichText::new(text).color(rust())).clicked() {
                    // §34: reuses the same command as the palette's broken-link
                    // entry, so creating from the panel and from the palette
                    // cannot diverge. The click side effect (creating the
                    // missing note) is surfaced as a status message, §UX-P2b.
                    self.selected_broken = Some(row.target.clone());
                    self.status = format!("Creating missing note {}", row.target);
                    self.execute(CommandId::CreateLinkTarget);
                }
            }
            ui.separator();
            ui.colored_label(muted(), "BACKLINKS");
            if backlinks.is_empty() {
                ui.colored_label(muted(), "none yet");
            }
            for id in &backlinks {
                self.note_row(ui, id.as_str().to_string(), id);
            }
            ui.separator();
            ui.colored_label(muted(), "UNLINKED MENTIONS");
            if unlinked.is_empty() {
                ui.colored_label(muted(), "none yet");
            }
            for mention in &unlinked {
                let preview = preview::for_note(&self.index, &mention.note, 8);
                let response = ui.button(format!(
                    "{} : {} (line {})",
                    mention.note, mention.snippet, mention.line
                ));
                if response.clicked() {
                    self.pending_line = Some((mention.note.clone(), mention.line));
                    self.open_note(&mention.note);
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        tab.view = ViewMode::Source;
                    }
                }
                hover_preview(ui, response, preview);
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.colored_label(muted(), "PROPERTIES");
                if ui.small_button("edit").clicked() {
                    self.open_properties();
                }
            });
            ui.horizontal_wrapped(|ui| {
                for tag in &note_tags {
                    if ui
                        .add(
                            egui::Button::new(RichText::new(format!("#{tag}")).color(accent()))
                                .fill(crate::tag())
                                .small(),
                        )
                        .clicked()
                    {
                        self.tag_filter = Some(tag.clone());
                    }
                }
            });
            ui.separator();
            ui.label(format!("History ({})", snapshots.len()));
            for stamp in &snapshots {
                if ui.small_button(stamp).clicked() {
                    let value = stamp.clone();
                    self.open_snapshot(&value);
                }
            }
        });

        // Status bar must be added before CentralPanel: egui gives remaining
        // space to the last top-level panel, so a bottom panel added after the
        // central one gets no visible height.
        let status_note = self.current_note_id();
        let saved_text = self
            .last_saved
            .as_ref()
            .filter(|(id, _)| status_note.as_ref().is_some_and(|note| note.as_str() == id))
            .map(|(_, at)| saved_label(at.elapsed().as_secs()));
        let words = self
            .tabs
            .get(self.active)
            .filter(|tab| tab.kind == TabKind::Note)
            .map(|tab| tab.content.split_whitespace().count())
            .unwrap_or(0);
        egui::Panel::bottom("status")
            .exact_size(30.0)
            .frame(egui::Frame::new().fill(panel()).inner_margin(Margin::symmetric(14, 4)))
            .show(ui, |ui| {
            // One line only: the 30 px bar must not clip (DESIGN §4).
            ui.horizontal(|ui| {
                // §UX-P3b: rely on the single error line for red; keep the
                // status text plain so nothing is drawn twice in rust.
                let status_color = if error.is_some() { text() } else { sage() };
                ui.colored_label(status_color, &self.status);
                ui.colored_label(muted(), format!("· {words} words"));
                if let Some(saved) = &saved_text {
                    ui.colored_label(muted(), format!("· saved {saved}"));
                }
                if let Some(error) = &error {
                    ui.colored_label(rust(), error);
                }
                // Reuse the vector the Context panel already read this frame;
                // a second filesystem read here can silently drop I/O errors.
                if current.is_some() {
                    ui.colored_label(muted(), format!("· {} snapshots", snapshots.len()));
                }
                if let Some((id, disk, deleted)) = &conflict {
                    // §47.4: the conflict belongs to a specific note, which may
                    // no longer be active. Never edit the active tab here.
                    let short: String = id.chars().take(40).collect();
                    let suffix = if id.chars().count() > 40 { "…" } else { "" };
                    ui.colored_label(amber(), format!("· external change in {short}{suffix}"));
                    if let Some(idx) = self.tabs.iter().position(|tab| {
                        tab.kind == TabKind::Note && tab.id == id.as_str()
                    }) {
                        // §UX-P1: a dirty tab whose note was deleted would get
                        // its buffer silently cleared by "Reload"; hide the
                        // button and say the buffer is kept. Overwrite stays
                        // the intentional recreate path.
                        let tab_dirty = self.tabs[idx].dirty;
                        let hide_reload = *deleted && tab_dirty;
                        if hide_reload {
                            self.status = "Note deleted — unsaved buffer kept".to_string();
                        } else if ui.small_button("Reload").clicked() {
                            if let Some(tab) = self.tabs.get_mut(idx) {
                                tab.content = disk.clone();
                                tab.loaded = disk.clone();
                                tab.dirty = false;
                                tab.last_edit = None;
                            }
                            self.conflict = None;
                            self.last_conflict_at = None;
                            self.status = "Reloaded from disk".to_string();
                        }
                        if ui.small_button("Overwrite").clicked() {
                            if let Some(tab) = self.tabs.get_mut(idx) {
                                tab.loaded = disk.clone();
                                tab.dirty = true;
                                tab.last_edit = Some(Instant::now());
                            }
                            self.conflict = None;
                            self.last_conflict_at = None;
                            self.flush_tab(idx);
                        }
                    }
                }
            });
        });

        // 6. Center: tabs + active view.
        let tab_count = self.tabs.len();
        let mut close_request = None;
        let mut navigate = None;
        let mut health_open = None;
        let mut health_create = None;
        let mut ctrl_navigate = None;
        egui::CentralPanel::default().show(ui, |ui| {
            let panel_rect = ui.available_rect_before_wrap();
            paint_gradient(ui, panel_rect);
            egui::ScrollArea::horizontal().id_salt("tab_strip").show(ui, |ui| {
                ui.horizontal(|ui| {
                    for idx in 0..tab_count {
                        let (title, kind) = {
                            let tab = &self.tabs[idx];
                            (tab.title.clone(), tab.kind)
                        };
                        let is_active = idx == self.active;
                        let response = if is_active {
                            ui.add(
                                egui::Button::new(RichText::new(&title).color(text()))
                                    .fill(raised()),
                            )
                        } else {
                            ui.button(&title)
                        };
                        let rect = response.rect;
                        if response.clicked() {
                            self.active = idx;
                        }
                        if is_active {
                            ui.painter().hline(
                                rect.x_range(),
                                rect.bottom(),
                                Stroke::new(2.0, accent()),
                            );
                        }
                        // DESIGN §4: the × is quiet chrome, shown only while
                        // the pointer is over the tab or the button itself.
                        // The small_button sits after item_spacing and keeps
                        // button_padding on both sides, so cover that whole
                        // region or the button vanishes under the cursor.
                        let spacing = ui.spacing();
                        let close_rect = egui::Rect::from_min_size(
                            rect.right_top(),
                            egui::Vec2::new(
                                spacing.item_spacing.x + spacing.button_padding.x * 2.0 + 12.0,
                                rect.height(),
                            ),
                        );
                        let show_close = is_active
                            || ui.rect_contains_pointer(rect)
                            || ui.rect_contains_pointer(close_rect);
                        // §UX-P3d: keyboard users cannot see a hover-only ×,
                        // so the close target includes the tab title.
                        if show_close && ui.small_button(format!("× {title}")).clicked() {
                            close_request = Some(idx);
                        }
                        let _ = kind;
                    }
                });
            });
            ui.separator();
            match self.tabs.get(self.active).map(|tab| tab.kind) {
                Some(TabKind::Note) => {
                    let mode = self.tabs.get(self.active).map(|tab| tab.view);
                    let bookmarked = self
                        .current_note_id()
                        .map(|id| self.bookmarks.contains(&id))
                        .unwrap_or(false);
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(mode == Some(ViewMode::Source), "Source")
                            .clicked()
                        {
                            if let Some(tab) = self.tabs.get_mut(self.active) {
                                tab.view = ViewMode::Source;
                            }
                        }
                        if ui
                            .selectable_label(mode == Some(ViewMode::Reading), "Reading")
                            .clicked()
                        {
                            if let Some(tab) = self.tabs.get_mut(self.active) {
                                tab.view = ViewMode::Reading;
                            }
                        }
                        ui.colored_label(muted(), "Ctrl+E");
                        if ui
                            .button(if bookmarked {
                                "Remove bookmark"
                            } else {
                                "Bookmark"
                            })
                            .clicked()
                        {
                            self.execute(CommandId::ToggleBookmark);
                        }
                    });
                    ui.separator();
                    match mode {
                        Some(ViewMode::Reading) => {
                            let display = self
                                .tabs
                                .get(self.active)
                                .map(|tab| {
                                    // Images introduced by whole-note/anchored
                                    // embeds only exist after expansion.
                                    let expanded = adapt_for_display(&tab.content, &self.index);
                                    self.prepare_images(&ctx, &expanded)
                                })
                                .unwrap_or_default();
                            ui.style_mut().url_in_tooltip = true;
                            let width = ui.available_width().min(820.0);
                            let height = ui.available_height();
                            egui::ScrollArea::vertical().id_salt("reading_view").show(ui, |ui| {
                                ui.allocate_ui_with_layout(
                                    egui::vec2(width, height),
                                    egui::Layout::top_down(egui::Align::LEFT)
                                        .with_cross_justify(true),
                                    |ui| {
                                        CommonMarkViewer::new()
                                            .alerts(callout_alerts())
                                            .show(ui, &mut self.markdown_cache, &display);
                                    },
                                );
                            });
                        }
                        Some(ViewMode::Source) => {
                            let height = ui.available_height();
                            let active_id = self.tabs.get(self.active).map(|tab| tab.id.clone());
                            let jump = self
                                .pending_line
                                .take()
                                .filter(|(id, _)| active_id.as_deref() == Some(id.as_str()));
                            let editor_id = active_id
                                .as_ref()
                                .map(|id| egui::Id::unique(("nexora_editor", id.clone())));
                            if let (Some((_, line)), Some(id)) = (jump.as_ref(), &editor_id) {
                                let content = self
                                    .tabs
                                    .get(self.active)
                                    .map(|tab| tab.content.clone())
                                    .unwrap_or_default();
                                let char_index: usize = content
                                    .lines()
                                    .take(line.saturating_sub(1))
                                    .map(|text| text.chars().count() + 1)
                                    .sum();
                                set_editor_cursor(&ctx, *id, char_index);
                            }
                            let scroll_to_line = jump.is_some();
                            // Capture keys on the frame the popup opens too:
                            // derive ownership from the caret, not last frame's
                            // popup state (§50 review).
                            // §GUI-2: the popup must never steal keys from a
                            // modal, and its caret is only meaningful while
                            // the editor actually has focus.
                            let editor_focused = editor_id.as_ref().is_some_and(|id| {
                                ctx.memory(|memory| memory.focused()) == Some(id.clone())
                            });
                            let popup_active = !self.modal_open()
                                && editor_focused
                                && editor_id
                                .as_ref()
                                .and_then(|id| egui::TextEdit::load_state(&ctx, id.clone()))
                                .and_then(|state| state.cursor.char_range())
                                .and_then(|range| {
                                    self.tabs.get(self.active).and_then(|tab| {
                                        partial_at(&tab.content, range.primary.index.into())
                                    })
                                })
                                .is_some_and(|link| {
                                    let key = (link.start, link.partial);
                                    self.link_dismissed.as_ref() != Some(&key)
                                });
                            if popup_active {
                                let flags = ctx.input(|input| {
                                    (
                                        input.key_pressed(Key::ArrowDown),
                                        input.key_pressed(Key::ArrowUp),
                                        input.key_pressed(Key::Enter),
                                        input.key_pressed(Key::Tab),
                                        input.key_pressed(Key::Escape),
                                    )
                                });
                                if flags.0 || flags.1 || flags.2 || flags.3 || flags.4 {
                                    popup_keys = Some(flags);
                                    // The editor must not consume these keys while
                                    // the autocomplete popup owns them.
                                    ctx.input_mut(|input| {
                                        input.events.retain(|event| {
                                            !matches!(
                                                event,
                                                egui::Event::Key {
                                                    key: Key::Enter
                                                        | Key::Tab
                                                        | Key::Escape
                                                        | Key::ArrowUp
                                                        | Key::ArrowDown,
                                                    pressed: true,
                                                    ..
                                                }
                                            )
                                        });
                                    });
                                }
                            }
                            // §55: list mechanics own Enter/Tab/Ctrl+Enter only
                            // when the editor is focused and the `[[` popup is
                            // not. Apply before drawing so egui sees the final
                            // text and caret in the same frame.
                            if !popup_active {
                                if let (Some(editor_id), Some(tab)) =
                                    (&editor_id, self.tabs.get_mut(self.active))
                                {
                                    if ctx.memory(|memory| memory.focused())
                                        == Some(editor_id.clone())
                                    {
                                        let char_count = tab.content.chars().count();
                                        let cursor = egui::TextEdit::load_state(
                                            &ctx,
                                            editor_id.clone(),
                                        )
                                        .and_then(|state| state.cursor.char_range())
                                        .map(|range| range.primary.index.into())
                                        .unwrap_or(char_count)
                                        .min(char_count);
                                        let flags = ctx.input(|input| {
                                            (
                                                input.key_pressed(Key::Enter)
                                                    && input.modifiers.ctrl,
                                                input.key_pressed(Key::Enter)
                                                    && !input.modifiers.ctrl
                                                    && !input.modifiers.shift,
                                                input.key_pressed(Key::Tab)
                                                    && !input.modifiers.ctrl
                                                    && !input.modifiers.shift,
                                                input.key_pressed(Key::Tab)
                                                    && input.modifiers.shift
                                                    && !input.modifiers.ctrl,
                                            )
                                        });
                                        // §55 list mechanics keep priority;
                                        // §57 pipe-table mechanics handle only
                                        // what the list transformations decline.
                                        let edit = if flags.0 {
                                            lists::toggle_task(&tab.content, cursor)
                                        } else if flags.1 {
                                            lists::on_enter(&tab.content, cursor)
                                                .or_else(|| tables::on_enter(&tab.content, cursor))
                                                .or_else(|| quotes::on_enter(&tab.content, cursor))
                                        } else if flags.2 {
                                            lists::indent(&tab.content, cursor, false).or_else(|| {
                                                tables::on_tab(&tab.content, cursor, false)
                                            })
                                        } else if flags.3 {
                                            lists::indent(&tab.content, cursor, true).or_else(|| {
                                                tables::on_tab(&tab.content, cursor, true)
                                            })
                                        } else {
                                            None
                                        };
                                        if let Some(edit) = edit {
                                            // Cursor-only key handling (Tab
                                            // between cells) must not mark the
                                            // note dirty or trigger autosave.
                                            let changed = edit.text != tab.content;
                                            tab.content = edit.text;
                                            if changed {
                                                tab.dirty = true;
                                                tab.last_edit = Some(Instant::now());
                                            }
                                            set_editor_cursor(&ctx, *editor_id, edit.cursor);
                                            // Prevent the editor from also
                                            // inserting a newline or tab.
                                            ctx.input_mut(|input| {
                                                input.events.retain(|event| {
                                                    !matches!(
                                                        event,
                                                        egui::Event::Key {
                                                            key: Key::Enter | Key::Tab,
                                                            pressed: true,
                                                            ..
                                                        }
                                                    )
                                                });
                                            });
                                        }
                                    }
                                }
                            }
                            let mut link_click = None;
                            let mut popup_data: Option<(usize, usize, String, egui::Pos2)> = None;
                            if let Some(tab) = self.tabs.get_mut(self.active) {
                                let builder = egui::TextEdit::multiline(&mut tab.content)
                                    .desired_rows(20)
                                    .min_size(egui::vec2(ui.available_width() - 8.0, height))
                                    .code_editor();
                                let builder = match &editor_id {
                                    Some(id) => builder.id(id.clone()),
                                    None => builder,
                                };
                                let output = builder.show(ui);
                                if output.response.changed() {
                                    tab.dirty = true;
                                    tab.last_edit = Some(Instant::now());
                                }
                                let pointer = ctx.input(|input| {
                                    if input.modifiers.ctrl
                                        && input.pointer.button_clicked(PointerButton::Primary)
                                    {
                                        input.pointer.latest_pos()
                                    } else {
                                        None
                                    }
                                });
                                if let Some(pos) = pointer {
                                    if output.response.rect.contains(pos) {
                                        let relative = pos - output.galley_pos;
                                        let byte = output.galley.cursor_from_pos(relative).index;
                                        link_click = Self::wikilink_at(&tab.content, byte.into());
                                    }
                                }
                                if scroll_to_line {
                                    ui.scroll_to_cursor(Some(egui::Align::Center));
                                }
                                let cursor_char: usize = output
                                    .cursor_range
                                    .map(|range| range.primary.index.into())
                                    .unwrap_or(0);
                                if let Some(part) = partial_at(&tab.content, cursor_char) {
                                    let rect = output
                                        .galley
                                        .pos_from_cursor(egui::text::CCursor::new(cursor_char));
                                    let pos = output.galley_pos + rect.min.to_vec2()
                                        + egui::vec2(0.0, rect.height());
                                    popup_data = Some((part.start, cursor_char, part.partial, pos));
                                }
                            }
                            if let Some(target) = link_click {
                                ctrl_navigate = Some(target);
                            }
                            if let Some((start, cursor, partial, pos)) = popup_data {
                                // §50 review: Escape must survive more than one
                                // frame. Do not reopen the same `[[` anchor.
                                if self.link_dismissed.as_ref()
                                    == Some(&(start, partial.clone()))
                                {
                                    self.link_popup = None;
                                } else {
                                    let previous = self
                                        .link_popup
                                        .as_ref()
                                        .filter(|popup| popup.partial == partial)
                                        .map(|popup| popup.selected)
                                        .unwrap_or(0);
                                    let mut ranked: Vec<(u32, NoteId)> = self
                                        .index
                                        .notes
                                        .keys()
                                        .filter_map(|id| {
                                            let path = fuzzy_score(&partial, id.as_str());
                                            let alias = self.alias_score(&partial, id);
                                            path.or(alias).map(|value| (value, id.clone()))
                                        })
                                        .collect();
                                    ranked.sort_by(|left, right| {
                                        right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1))
                                    });
                                    let suggestions: Vec<NoteId> = ranked
                                        .into_iter()
                                        .take(8)
                                        .map(|(_, id)| id)
                                        .collect();
                                    // §50: keep the popup even with no matches so
                                    // the user gets an explicit "no matching notes"
                                    // state instead of a silent no-op.
                                    // Clamp to the actual list, not a magic 7.
                                    let selected =
                                        previous.min(suggestions.len().saturating_sub(1));
                                    self.link_popup = Some(LinkPopup {
                                        start,
                                        cursor,
                                        partial,
                                        suggestions,
                                        selected,
                                        pos,
                                    });
                                }
                            } else {
                                self.link_popup = None;
                                self.link_dismissed = None;
                            }
                        }
                        None => {}
                    }
                }
                Some(TabKind::Graph) => self.draw_graph(ui),
                Some(TabKind::Canvas) => self.draw_canvas(ui),
                Some(TabKind::Base) => self.draw_base(ui),
                Some(TabKind::Health) => {
                    let (open, create) = self.draw_health(ui);
                    health_open = open;
                    health_create = create;
                }
                Some(TabKind::Search) => {
                    navigate = self.draw_search(ui);
                }
                None => {
                    ui.label("No note open. Press Ctrl+N or choose a note on the left.");
                }
            }
        });
        // §50 review: a popup belongs to one Source note. Switching tabs or
        // to Reading must not leave it floating over another view.
        let source_note = self
            .tabs
            .get(self.active)
            .is_some_and(|tab| tab.kind == TabKind::Note && tab.view == ViewMode::Source);
        // §GUI-2: a modal owns the screen; hide the popup so it cannot draw
        // over the dialog or interpret dialog keystrokes.
        if !source_note || self.modal_open() {
            self.link_popup = None;
            self.link_dismissed = None;
        }
        if let Some(popup) = self.link_popup.as_ref() {
            let suggestions = popup.suggestions.clone();
            let pos = popup.pos;
            let selected = popup.selected;
            let dismissal_key = (popup.start, popup.partial.clone());
            let mut open = true;
            let mut move_delta = 0i32;
            let mut commit = false;
            let mut dismiss = false;
            let mut chosen: Option<NoteId> = None;
            egui::Window::new("Link autocomplete")
                .open(&mut open)
                .current_pos(pos + egui::vec2(0.0, 4.0))
                .resizable(false)
                .collapsible(false)
                .show(&ctx, |ui| {
                    let pressed =
                        popup_keys.unwrap_or((false, false, false, false, false));
                    if pressed.0 {
                        move_delta = 1;
                    }
                    if pressed.1 {
                        move_delta = -1;
                    }
                    if pressed.2 || pressed.3 {
                        commit = true;
                    }
                    if pressed.4 {
                        dismiss = true;
                    }
                    if suggestions.is_empty() {
                        ui.colored_label(muted(), "No matching notes");
                    }
                    for (index, id) in suggestions.iter().enumerate() {
                        if ui.selectable_label(index == selected, id.as_str()).clicked() {
                            chosen = Some(id.clone());
                        }
                    }
                });
            if let Some(target) = chosen {
                self.insert_link(&ctx, target.as_str());
            } else if commit {
                if let Some(target) = self
                    .link_popup
                    .as_ref()
                    .and_then(|popup| popup.suggestions.get(popup.selected))
                    .cloned()
                {
                    self.insert_link(&ctx, target.as_str());
                }
            } else if move_delta != 0 {
                if let Some(popup) = self.link_popup.as_mut() {
                    // §50: an empty suggestion list has nothing to wrap over.
                    let length = popup.suggestions.len();
                    if length > 0 {
                        popup.selected = (popup.selected as i32 + move_delta)
                            .rem_euclid(length as i32) as usize;
                    }
                }
            } else if dismiss || !open {
                self.link_popup = None;
                // Remember the dismissed anchor so the next frame does not
                // immediately recreate the popup (§50 review).
                self.link_dismissed = Some(dismissal_key);
            }
        }
        if let Some(idx) = close_request {
            self.close_tab(idx);
        }
        if let Some(id) = navigate {
            self.open_note(&id);
        }
        if let Some(target) = health_create {
            self.selected_broken = Some(target);
            self.execute(CommandId::CreateLinkTarget);
        }
        if let Some((id, line)) = health_open {
            self.open_note(&id);
            if let Some(line) = line {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.view = ViewMode::Source;
                }
                self.pending_line = Some((id.clone(), line));
                self.status = format!("Jumped to {id}:{line}");
            }
        }
        if let Some(target) = ctrl_navigate {
            if self.index.notes.contains_key(&target) {
                self.selected_broken = None;
                self.open_note(&target);
            } else {
                self.selected_broken = Some(target.clone());
                self.status = format!("Broken link: {target} (Ctrl+P → Create note at target)");
            }
        }

        // 7. Command palette and quick capture modals.
        let palette_commands = commands::filter(&self.palette_query);
        let mut chosen_command = None;
        if self.palette_open {
            // A changed query produces a different ranking; restore the first
            // row so Enter targets the top match (§67 review).
            if self.palette_selection_key != self.palette_query {
                self.palette_selected = 0;
                self.palette_selection_key = self.palette_query.clone();
            }
            self.palette_selected = self.palette_selected.min(palette_commands.len().saturating_sub(1));
            let mut move_up = false;
            let mut move_down = false;
            ctx.input(|input| {
                move_up = input.key_pressed(Key::ArrowUp);
                move_down = input.key_pressed(Key::ArrowDown);
            });
            if move_up {
                self.palette_selected = self.palette_selected.saturating_sub(1);
            }
            if move_down && self.palette_selected + 1 < palette_commands.len() {
                self.palette_selected += 1;
            }
            if input_enter(&ctx) {
                if let Some(command) = palette_commands.get(self.palette_selected) {
                    chosen_command = Some(command.id);
                    // The command may open a modal this same frame. Consume
                    // Enter so that modal's own input_enter cannot confirm an
                    // empty form before the user has typed anything.
                    ctx.input_mut(|input| {
                        // input_enter ignores modifiers, so drop every pressed
                        // Enter event, not just the unmodified one.
                        input.events.retain(|event| {
                            !matches!(
                                event,
                                egui::Event::Key {
                                    key: Key::Enter,
                                    pressed: true,
                                    ..
                                }
                            )
                        });
                    });
                }
            }
            let mut palette_open = true;
            let mut palette_cancel = false;
            egui::Window::new("Command palette")
                .open(&mut palette_open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 120.0))
                .show(&ctx, |ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.palette_query)
                            .hint_text("Type a command...")
                            .desired_width(320.0),
                    );
                    response.request_focus();
                    if ui.small_button("Cancel").clicked() {
                        palette_cancel = true;
                    }
                    egui::ScrollArea::vertical().id_salt("palette_commands").max_height(260.0).show(ui, |ui| {
                        if palette_commands.is_empty() {
                            ui.colored_label(muted(), "No matching commands");
                        }
                        for (index, command) in palette_commands.iter().enumerate() {
                            let label = match command.shortcut {
                                Some(shortcut) => format!("{}   {}", command.label, shortcut),
                                None => command.label.to_string(),
                            };
                            if ui.selectable_label(index == self.palette_selected, label).clicked() {
                                chosen_command = Some(command.id);
                            }
                        }
                    });
                });
            self.palette_open = palette_open && !keys.escape && !palette_cancel;
        }
        if let Some(command) = chosen_command {
            self.execute(command);
            self.palette_open = false;
        }
        if !self.palette_open {
            self.palette_selection_key.clear();
        }

        if self.capture_open {
            let mut capture_open = true;
            let mut cancel = false;
            let mut do_capture = false;
            egui::Window::new("Quick capture")
                .open(&mut capture_open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(&ctx, |ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.capture_text)
                            .hint_text("Capture to Inbox...")
                            .desired_width(360.0),
                    );
                    response.request_focus();
                    ui.horizontal(|ui| {
                        if ui.button("Add").clicked() || input_enter(&ctx) {
                            do_capture = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                });
            self.capture_open = capture_open && !keys.escape && !cancel;
            if do_capture {
                self.capture();
            }
        }

        if self.rename_input_open {
            let old = self
                .current_note_id()
                .map(|id| id.as_str().to_string())
                .unwrap_or_default();
            let mut open = true;
            let mut preview = false;
            let mut cancel = false;
            egui::Window::new("Rename note")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(&ctx, |ui| {
                    ui.colored_label(muted(), format!("from: {old}"));
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.rename_new)
                            .hint_text("new vault path")
                            .desired_width(360.0),
                    );
                    response.request_focus();
                    if ui.button("Preview").clicked() || input_enter(&ctx) {
                        preview = true;
                    }
                    if ui.button("Cancel").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            self.rename_input_open = open && !cancel && !preview;
            if preview {
                self.preview_rename();
            }
        }

        if self.rename_confirm_open {
            let summary = self
                .rename_plan
                .as_ref()
                .map(|plan| {
                    let links: usize = plan.edits.iter().map(|edit| edit.replacements).sum();
                    format!(
                        "{} → {}\n{} files · {links} link replacements\nEvery touched note is snapshotted first.",
                        plan.old, plan.new, plan.edits.len()
                    )
                })
                .unwrap_or_default();
            let mut open = true;
            let mut confirm = false;
            let mut cancel = false;
            egui::Window::new("Confirm rename")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(&ctx, |ui| {
                    ui.label(&summary);
                    if ui.button("Confirm").clicked() {
                        confirm = true;
                    }
                    if ui.button("Cancel").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            self.rename_confirm_open = open && !cancel && !confirm;
            if cancel {
                self.rename_plan = None;
            }
            if confirm {
                self.apply_rename();
            }
        }

        if self.properties_open {
            let mut window_open = true;
            let mut save = false;
            let mut cancel = false;
            // §81: a refused save keeps this modal open, so the cause must be
            // visible inside it, not only in the status bar.
            let inline_error = self.error.clone();
            egui::Window::new("Properties")
                .open(&mut window_open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(&ctx, |ui| {
                    if let Some(error) = &inline_error {
                        ui.colored_label(rust(), error);
                    }
                    ui.label("created");
                    ui.add(egui::TextEdit::singleline(&mut self.prop_created).desired_width(360.0));
                    ui.label("modified");
                    ui.add(egui::TextEdit::singleline(&mut self.prop_modified).desired_width(360.0));
                    ui.label("aliases (comma separated)");
                    ui.add(egui::TextEdit::singleline(&mut self.prop_aliases).desired_width(360.0));
                    ui.label("tags (comma separated)");
                    ui.add(egui::TextEdit::singleline(&mut self.prop_tags).desired_width(360.0));
                    ui.colored_label(muted(), "Other frontmatter lines are preserved as-is.");
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            save = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                });
            if cancel || keys.escape || !window_open {
                self.properties_open = false;
            } else if save {
                self.save_properties();
            }
        }

        if self.quick_open {
            let results = self.quick_results.clone();
            let mut window_open = true;
            let mut refresh = false;
            let mut chosen: Option<NoteId> = None;
            let mut move_delta = 0i32;
            let mut submit = false;
            egui::Window::new("Quick open")
                .open(&mut window_open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 120.0))
                .show(&ctx, |ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.quick_query)
                            .hint_text("Type part of a title or path...")
                            .desired_width(420.0),
                    );
                    response.request_focus();
                    if response.changed() {
                        refresh = true;
                    }
                    let pressed = ctx.input(|input| {
                        (
                            input.key_pressed(Key::ArrowDown),
                            input.key_pressed(Key::ArrowUp),
                            input.key_pressed(Key::Enter),
                        )
                    });
                    if pressed.0 {
                        move_delta = 1;
                    }
                    if pressed.1 {
                        move_delta = -1;
                    }
                    if pressed.2 {
                        submit = true;
                    }
                    for (index, id) in results.iter().enumerate() {
                        let preview = preview::for_note(&self.index, id, 8);
                        let response =
                            ui.selectable_label(index == self.quick_selected, id.as_str());
                        if response.clicked() {
                            chosen = Some(id.clone());
                        }
                        hover_preview(ui, response, preview);
                    }
                    if results.is_empty() {
                        ui.colored_label(muted(), "No matching notes");
                    }
                });
            if refresh {
                self.refresh_quick();
            }
            if move_delta != 0 && !self.quick_results.is_empty() {
                let length = self.quick_results.len() as i32;
                self.quick_selected =
                    (self.quick_selected as i32 + move_delta).rem_euclid(length) as usize;
            }
            if submit {
                if let Some(id) = self.quick_results.get(self.quick_selected) {
                    chosen = Some(id.clone());
                }
            }
            if let Some(id) = chosen {
                self.open_note(&id);
                window_open = false;
            }
            self.quick_open = window_open && !keys.escape;
        }

        if self.outline_open {
            let results = self.outline_results.clone();
            let mut window_open = true;
            let mut refresh = false;
            let mut chosen: Option<(NoteId, usize)> = None;
            let mut move_delta = 0i32;
            let mut submit = false;
            egui::Window::new("Go to heading")
                .open(&mut window_open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 120.0))
                .show(&ctx, |ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.outline_query)
                            .hint_text("Filter headings...")
                            .desired_width(420.0),
                    );
                    response.request_focus();
                    if response.changed() {
                        refresh = true;
                    }
                    let pressed = ctx.input(|input| {
                        (
                            input.key_pressed(Key::ArrowDown),
                            input.key_pressed(Key::ArrowUp),
                            input.key_pressed(Key::Enter),
                        )
                    });
                    if pressed.0 {
                        move_delta = 1;
                    }
                    if pressed.1 {
                        move_delta = -1;
                    }
                    if pressed.2 {
                        submit = true;
                    }
                    for (index, (_, line, title)) in results.iter().enumerate() {
                        let label = format!("{line}: {title}");
                        if ui.selectable_label(index == self.outline_selected, label).clicked() {
                            let entry = &results[index];
                            chosen = Some((entry.0.clone(), entry.1));
                        }
                    }
                    if results.is_empty() {
                        ui.colored_label(muted(), "No headings in this note");
                    }
                });
            if refresh {
                self.refresh_outline();
            }
            if move_delta != 0 && !self.outline_results.is_empty() {
                let length = self.outline_results.len() as i32;
                self.outline_selected =
                    (self.outline_selected as i32 + move_delta).rem_euclid(length) as usize;
            }
            if submit {
                if let Some(entry) = self.outline_results.get(self.outline_selected) {
                    chosen = Some((entry.0.clone(), entry.1));
                }
            }
            if let Some((id, line)) = chosen {
                self.open_note(&id);
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.view = ViewMode::Source;
                }
                self.pending_line = Some((id, line));
                window_open = false;
            }
            self.outline_open = window_open && !keys.escape;
        }

        if self.bookmarks_open {
            let mut open = true;
            let mut cancel = false;
            let mut action: Option<(NoteId, BookmarkAction)> = None;
            let rows = self.bookmarks.ids.clone();
            let inline_error = self.error.clone();
            egui::Window::new("Bookmarks")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(420.0, 320.0))
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 150.0))
                .show(&ctx, |ui| {
                    if let Some(error) = &inline_error {
                        ui.colored_label(rust(), error);
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("bookmark_rows")
                        .show(ui, |ui| {
                            if rows.is_empty() {
                                ui.colored_label(muted(), "No bookmarks yet");
                            }
                            for id in &rows {
                                ui.horizontal(|ui| {
                                    if ui.button(id.as_str()).clicked() {
                                        action = Some((id.clone(), BookmarkAction::Open));
                                    }
                                    if ui.small_button("↑").clicked() {
                                        action = Some((id.clone(), BookmarkAction::Up));
                                    }
                                    if ui.small_button("↓").clicked() {
                                        action = Some((id.clone(), BookmarkAction::Down));
                                    }
                                    if ui.small_button("Remove").clicked() {
                                        action = Some((id.clone(), BookmarkAction::Remove));
                                    }
                                });
                            }
                        });
                    if ui.button("Close").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            if let Some((id, kind)) = action {
                match kind {
                    BookmarkAction::Open => {
                        self.open_note(&id);
                        self.bookmarks_open = false;
                        self.error = None;
                    }
                    BookmarkAction::Up | BookmarkAction::Down | BookmarkAction::Remove => {
                        let delta = match kind {
                            BookmarkAction::Up => -1,
                            BookmarkAction::Down => 1,
                            _ => 0,
                        };
                        let mut next = self.bookmarks.clone();
                        let changed = if kind == BookmarkAction::Remove {
                            next.remove(&id)
                        } else {
                            next.move_by(&id, delta)
                        };
                        if changed {
                            match bookmarks::save(&self.bookmarks_path, &next) {
                                Ok(()) => {
                                    self.bookmarks = next;
                                    self.status = match kind {
                                        BookmarkAction::Remove => format!("Removed bookmark {id}"),
                                        _ => format!("Moved bookmark {id}"),
                                    };
                                    self.error = None;
                                }
                                Err(error) => self.error = Some(error.to_string()),
                            }
                        }
                    }
                }
            } else if cancel || !open {
                self.bookmarks_open = false;
                self.error = None;
            }
        }

        if let Some(modal) = self.snapshot_modal.as_ref() {
            let title = format!("Snapshot {}", modal.stamp);
            let dirty = modal.dirty;
            let rows: Vec<(DiffKind, String, Option<usize>, Option<usize>)> = modal
                .rows
                .iter()
                .map(|row| (row.kind, row.text.clone(), row.old_line, row.new_line))
                .collect();
            let mut open = true;
            let mut confirm = false;
            let mut cancel = false;
            egui::Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(640.0, 480.0))
                .show(&ctx, |ui| {
                    if dirty {
                        ui.colored_label(amber(), "Unsaved changes — save or discard before restoring");
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("snapshot_diff")
                        .max_height(370.0)
                        .show(ui, |ui| {
                            egui::Grid::new("snapshot_diff_grid").show(ui, |ui| {
                                for (kind, text, old_line, new_line) in &rows {
                                    let marker = match kind {
                                        DiffKind::Equal => " ",
                                        DiffKind::Added => "+",
                                        DiffKind::Removed => "-",
                                    };
                                    let color = match kind {
                                        DiffKind::Equal => muted(),
                                        DiffKind::Added => sage(),
                                        DiffKind::Removed => rust(),
                                    };
                                    ui.colored_label(color, marker);
                                    ui.colored_label(color, text);
                                    ui.colored_label(
                                        muted(),
                                        format!("{} / {}", old_line.unwrap_or(0), new_line.unwrap_or(0)),
                                    );
                                    ui.end_row();
                                }
                            });
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!dirty, egui::Button::new("Restore")).clicked() {
                            confirm = true;
                        }
                        if ui.button("Close").clicked() || keys.escape {
                            cancel = true;
                        }
                    });
                });
            if cancel || !open {
                self.snapshot_modal = None;
            } else if confirm {
                self.confirm_restore();
            }
        }

        if self.template_open {
            let mut open = true;
            let mut cancel = false;
            let mut chosen: Option<Template> = None;
            let mut edit_target: Option<Template> = None;
            let mut delete_confirm: Option<String> = None;
            let mut new_request = false;
            let mut clear_error = false;
            let rows = self.templates.clone();
            let inline_error = self.error.clone();
            let mut delete_target = self.template_delete.clone();
            let mode = self.template_picker_mode;
            let title = if mode == TemplatePickerMode::Insert {
                "Insert template into note"
            } else {
                "New note from template"
            };
            egui::Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(420.0, 320.0))
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 140.0))
                .show(&ctx, |ui| {
                    if let Some(error) = &inline_error {
                        ui.colored_label(rust(), error);
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("template_rows")
                        .show(ui, |ui| {
                            for template in &rows {
                                ui.horizontal(|ui| {
                                    if ui.button(&template.name).clicked() {
                                        chosen = Some(template.clone());
                                    }
                                    if ui.small_button("Edit").clicked() {
                                        edit_target = Some(template.clone());
                                        clear_error = true;
                                    }
                                    if template.name != templates::DAILY_TEMPLATE
                                        && ui.small_button("Delete").clicked()
                                    {
                                        delete_target = Some(template.name.clone());
                                        clear_error = true;
                                    }
                                });
                                ui.colored_label(
                                    muted(),
                                    template.body.lines().next().unwrap_or("(empty)"),
                                );
                                if delete_target.as_deref() == Some(template.name.as_str()) {
                                    ui.horizontal(|ui| {
                                        ui.colored_label(rust(), "Delete this template?");
                                        if ui.small_button("Delete").clicked() {
                                            delete_confirm = Some(template.name.clone());
                                        }
                                        if ui.small_button("Cancel").clicked() {
                                            delete_target = None;
                                            clear_error = true;
                                        }
                                    });
                                }
                            }
                        });
                    if ui.button("New template").clicked() {
                        new_request = true;
                        clear_error = true;
                    }
                    if ui.button("Cancel").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            self.template_delete = delete_target;
            if clear_error {
                self.error = None;
            }
            if let Some(template) = chosen {
                let applied = if mode == TemplatePickerMode::Insert {
                    self.insert_template(&ctx, &template)
                } else {
                    self.create_from_template(&template)
                };
                if applied {
                    self.template_open = false;
                }
            } else if let Some(template) = edit_target {
                self.template_edit = Some(template.name.clone());
                self.template_name = template.name;
                self.template_body = template.body;
                self.template_open = false;
                self.template_editor_open = true;
            } else if new_request {
                self.template_edit = None;
                self.template_name.clear();
                self.template_body.clear();
                self.template_open = false;
                self.template_editor_open = true;
            } else if let Some(name) = delete_confirm {
                match templates::remove(&self.templates, &name) {
                    Ok(updated) => match templates::save(&templates_path(), &updated) {
                        Ok(()) => {
                            self.templates = updated;
                            self.template_delete = None;
                            self.status = format!("Deleted template {name}");
                            self.error = None;
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    },
                    Err(error) => self.error = Some(error.to_string()),
                }
            } else if cancel || !open {
                self.template_open = false;
                self.template_delete = None;
                self.error = None;
            }
        }

        if self.template_editor_open {
            let mut open = true;
            let mut cancel = false;
            let mut submit = false;
            let inline_error = self.error.clone();
            let old = self.template_edit.clone();
            let daily = old.as_deref() == Some(templates::DAILY_TEMPLATE);
            egui::Window::new(if old.is_some() { "Edit template" } else { "New template" })
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(420.0, 320.0))
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 140.0))
                .show(&ctx, |ui| {
                    if let Some(error) = &inline_error {
                        ui.colored_label(rust(), error);
                    }
                    ui.label("name");
                    ui.add_enabled(
                        !daily,
                        egui::TextEdit::singleline(&mut self.template_name)
                            .hint_text("template name")
                            .desired_width(380.0),
                    );
                    ui.label("body");
                    egui::ScrollArea::vertical()
                        .id_salt("template_body")
                        .max_height(210.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.template_body)
                                    .desired_rows(10)
                                    .desired_width(380.0),
                            );
                        });
                    if ui.button("Save").clicked() {
                        submit = true;
                    }
                    if ui.button("Cancel").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            if submit {
                let name = self.template_name.trim().to_string();
                let body = self.template_body.clone();
                let updated = old.as_ref().and_then(|old_name| (old_name != &name).then_some(old_name));
                let result = match updated {
                    Some(old_name) => templates::rename(&self.templates, old_name, &name)
                        .and_then(|list| {
                            templates::upsert(
                                &list,
                                Template {
                                    name: name.clone(),
                                    body: body.clone(),
                                },
                            )
                        }),
                    None => templates::upsert(
                        &self.templates,
                        Template {
                            name: name.clone(),
                            body: body.clone(),
                        },
                    ),
                };
                match result.and_then(|list| {
                    templates::save(&templates_path(), &list).map(|()| list)
                }) {
                    Ok(list) => {
                        self.templates = list;
                        self.template_editor_open = false;
                        self.template_edit = None;
                        self.status = format!("Saved template {name}");
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            } else if cancel || !open {
                self.template_editor_open = false;
                self.template_edit = None;
                self.error = None;
            }
        }

        if let Some(mode) = self.composer_mode {
            let mut open = true;
            let mut cancel = false;
            let mut action: Option<ComposerAction> = None;
            let targets = self.merge_targets.clone();
            // §48: render the current failure inside the window so the user
            // sees why it stayed open instead of only in the status bar.
            let inline_error = self.error.clone();
            let title = match mode {
                ComposerMode::Extract => "Extract lines to new note",
                ComposerMode::Merge => "Merge note into another",
            };
            egui::Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 150.0))
                .show(&ctx, |ui| {
                    if let Some(error) = &inline_error {
                        ui.colored_label(rust(), error);
                    }
                    match mode {
                        ComposerMode::Extract => {
                            ui.horizontal(|ui| {
                                ui.label("from");
                                ui.text_edit_singleline(&mut self.extract_first);
                                ui.label("to");
                                ui.text_edit_singleline(&mut self.extract_last);
                            });
                            ui.horizontal(|ui| {
                                ui.label("new note");
                                ui.text_edit_singleline(&mut self.extract_new);
                            });
                            if ui.button("Extract").clicked() {
                                action = Some(ComposerAction::Extract);
                            }
                        }
                        ComposerMode::Merge => {
                            ui.colored_label(muted(), "Merge current note into:");
                            egui::ScrollArea::vertical()
                                .id_salt("composer_merge_targets")
                                .max_height(260.0)
                                .show(ui, |ui| {
                                    for id in &targets {
                                        if ui.button(id.as_str()).clicked() {
                                            action = Some(ComposerAction::Merge(id.clone()));
                                        }
                                    }
                                });
                        }
                    }
                    ui.separator();
                    if ui.button("Cancel").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            if let Some(action) = action {
                // §48: keep the window open unless the action actually
                // succeeded, so bad line numbers or I/O errors stay visible
                // and the entered values are preserved.
                let mut succeeded = false;
                match action {
                    ComposerAction::Extract => {
                        let source = self.current_note_id();
                        let first = self.extract_first.trim().parse::<usize>().ok();
                        let last = self.extract_last.trim().parse::<usize>().ok();
                        let new_id = NoteId::new(self.extract_new.trim());
                        match (source, first, last) {
                            (Some(source), Some(first), Some(last)) => {
                                // §78: never rewrite a source with unsaved edits.
                                if self.tabs.iter().any(|tab| {
                                    tab.kind == TabKind::Note
                                        && tab.id == source.as_str()
                                        && tab.dirty
                                }) {
                                    self.status = "Save or discard changes before extracting".to_string();
                                    self.error = Some(self.status.clone());
                                } else {
                                    match composer::apply_extract(
                                        &self.root,
                                        &self.index,
                                        &source,
                                        &new_id,
                                        first,
                                        last,
                                    ) {
                                        Ok(plan) => {
                                            self.reload_index();
                                            // The open source tab must see the edited
                                            // bytes, or a later edit looks like a GM-4
                                            // external change (§54/§78).
                                            self.refresh_tab_after_write(&source, &plan.updated_source);
                                            self.open_note(&plan.new_id);
                                            self.status = format!("Extracted lines {first}-{last} into {}", plan.new_id);
                                            succeeded = true;
                                        }
                                        Err(error) => self.error = Some(error.to_string()),
                                    }
                                }
                            }
                            _ => self.error = Some("Line numbers must be integers".to_string()),
                        }
                    }
                    ComposerAction::Merge(target) => {
                        if let Some(source) = self.current_note_id() {
                            // §78: a merge rewrites arbitrary witness notes; refuse
                            // when any note tab is dirty so close_tab cannot flush
                            // a stale buffer over the deleted source.
                            let dirty = self.tabs.iter().any(|tab| tab.kind == TabKind::Note && tab.dirty);
                            if !dirty {
                            match composer::apply_merge(&self.root, &self.index, &source, &target) {
                                Ok(plan) => {
                                    self.reload_index();
                                    for (id, bytes) in &plan.changes {
                                        self.refresh_tab_after_write(id, bytes);
                                    }
                                    if let Some(pos) = self.tabs.iter().position(|tab| {
                                        tab.kind == TabKind::Note && tab.id == source.as_str()
                                    }) {
                                        self.close_tab(pos);
                                    }
                                    self.open_note(&target);
                                    self.status = format!(
                                        "Merged {source} into {target} ({} links rewritten)",
                                        plan.rewritten
                                    );
                                    succeeded = true;
                                }
                                Err(error) => self.error = Some(error.to_string()),
                            }
                            } else {
                                self.status = "Save or discard changes before merging".to_string();
                                self.error = Some(self.status.clone());
                            }
                        } else {
                            self.error = Some("No source note open for merge".to_string());
                        }
                    }
                }
                if succeeded {
                    self.composer_mode = None;
                }
            } else if cancel || !open {
                self.composer_mode = None;
            }
        }

        if self.save_query_open {
            let mut open = true;
            let mut cancel = false;
            let mut confirm = false;
            let current = self.search_query.clone();
            let inline_error = self.error.clone();
            egui::Window::new("Save current query")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 150.0))
                .show(&ctx, |ui| {
                    ui.colored_label(muted(), &current);
                    if let Some(error) = &inline_error {
                        ui.colored_label(rust(), error);
                    }
                    ui.horizontal(|ui| {
                        ui.label("name");
                        ui.text_edit_singleline(&mut self.query_name);
                    });
                    if ui.button("Save").clicked() || input_enter(&ctx) {
                        confirm = true;
                    }
                    if ui.button("Cancel").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            if confirm {
                let name = self.query_name.trim().to_string();
                if name.is_empty() {
                    self.error = Some("Query name is empty".to_string());
                } else {
                    // §81: do not advance memory ahead of a successful write.
                    let updated = saved_queries::upsert(&self.saved_queries, &name, &current);
                    match saved_queries::save(&saved_queries_path(), &updated) {
                        Ok(()) => {
                            self.saved_queries = updated;
                            self.status = format!("Saved query {name}");
                            self.error = None;
                            self.save_query_open = false;
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                }
            } else if cancel || !open {
                self.save_query_open = false;
            }
        }

        if self.apply_query_open {
            let mut open = true;
            let mut cancel = false;
            let mut chosen: Option<String> = None;
            let mut rename_action: Option<(String, String)> = None;
            let mut delete_confirm: Option<String> = None;
            let mut clear_error = false;
            let rows = self.saved_queries.clone();
            let inline_error = self.error.clone();
            let mut rename_target = self.saved_query_rename.as_ref().map(|(old, _)| old.clone());
            let mut rename_draft = self
                .saved_query_rename
                .as_ref()
                .map(|(_, draft)| draft.clone())
                .unwrap_or_default();
            let mut delete_target = self.saved_query_delete.clone();

            // §82: selection belongs to one list snapshot.
            let key = saved_queries::selection_key(&rows);
            if self.saved_query_selection_key != key {
                self.saved_query_selected = 0;
                self.saved_query_selection_key = key;
            }
            self.saved_query_selected = self
                .saved_query_selected
                .min(rows.len().saturating_sub(1));
            let rename_editor_id = egui::Id::unique(("saved_query_rename",));
            let rename_focused = ctx.memory(|memory| memory.focused() == Some(rename_editor_id));
            let mut scroll_to_selected = false;
            if rename_target.is_none() && delete_target.is_none() && !rename_focused {
                let (up, down, enter) = ctx.input(|input| {
                    (
                        input.key_pressed(Key::ArrowUp),
                        input.key_pressed(Key::ArrowDown),
                        input.key_pressed(Key::Enter),
                    )
                });
                if (up || down) && !rows.is_empty() {
                    let length = rows.len();
                    self.saved_query_selected = if up {
                        (self.saved_query_selected + length - 1) % length
                    } else {
                        (self.saved_query_selected + 1) % length
                    };
                    scroll_to_selected = true;
                }
                if enter && !rows.is_empty() {
                    chosen = rows.get(self.saved_query_selected).map(|query| query.name.clone());
                }
                if up || down || enter {
                    ctx.input_mut(|input| {
                        input.events.retain(|event| {
                            !matches!(
                                event,
                                egui::Event::Key {
                                    key: Key::ArrowUp | Key::ArrowDown | Key::Enter,
                                    pressed: true,
                                    ..
                                }
                            )
                        });
                    });
                }
            }

            egui::Window::new("Apply saved query")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(420.0, 320.0))
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 150.0))
                .show(&ctx, |ui| {
                    if let Some(error) = &inline_error {
                        ui.colored_label(rust(), error);
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("saved_query_rows")
                        .show(ui, |ui| {
                            for (index, query) in rows.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    let response = ui.selectable_label(
                                        index == self.saved_query_selected,
                                        &query.name,
                                    );
                                    // A click must never silently commit an
                                    // in-progress rename or delete (§65/§82).
                                    if rename_target.is_none()
                                        && delete_target.is_none()
                                        && response.clicked()
                                    {
                                        self.saved_query_selected = index;
                                        chosen = Some(query.name.clone());
                                    }
                                    if scroll_to_selected && index == self.saved_query_selected {
                                        response.scroll_to_me(Some(egui::Align::Center));
                                    }
                                    if rename_target.as_deref() != Some(query.name.as_str())
                                        && delete_target.is_none()
                                        && ui.small_button("Rename").clicked()
                                    {
                                        rename_target = Some(query.name.clone());
                                        rename_draft = query.name.clone();
                                        clear_error = true;
                                    }
                                    if rename_target.is_none()
                                        && ui.small_button("Delete").clicked()
                                    {
                                        delete_target = Some(query.name.clone());
                                        clear_error = true;
                                    }
                                });
                                ui.colored_label(muted(), &query.text);
                                if rename_target.as_deref() == Some(query.name.as_str()) {
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(&mut rename_draft)
                                                .id(rename_editor_id),
                                        );
                                        if ui.small_button("OK").clicked() {
                                            rename_action =
                                                Some((query.name.clone(), rename_draft.clone()));
                                        }
                                        if ui.small_button("Cancel").clicked() {
                                            rename_target = None;
                                            clear_error = true;
                                        }
                                    });
                                }
                                if delete_target.as_deref() == Some(query.name.as_str()) {
                                    ui.horizontal(|ui| {
                                        ui.colored_label(rust(), "Delete this saved query?");
                                        if ui.small_button("Delete").clicked() {
                                            delete_confirm = Some(query.name.clone());
                                        }
                                        if ui.small_button("Cancel").clicked() {
                                            delete_target = None;
                                            clear_error = true;
                                        }
                                    });
                                }
                            }
                        });
                    if ui.button("Cancel").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            self.saved_query_rename = rename_target.map(|old| (old, rename_draft));
            self.saved_query_delete = delete_target;
            if clear_error {
                self.error = None;
            }

            if let Some((old, new)) = rename_action {
                match saved_queries::rename(&self.saved_queries, &old, &new) {
                    Ok(updated) => {
                        match saved_queries::save(&saved_queries_path(), &updated) {
                            Ok(()) => {
                                self.saved_queries = updated;
                                self.saved_query_rename = None;
                                self.status =
                                    format!("Renamed saved query {old} to {}", new.trim());
                                self.error = None;
                            }
                            Err(error) => self.error = Some(error.to_string()),
                        }
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            } else if let Some(name) = delete_confirm {
                let updated = saved_queries::remove(&self.saved_queries, &name);
                match saved_queries::save(&saved_queries_path(), &updated) {
                    Ok(()) => {
                        self.saved_queries = updated;
                        self.saved_query_delete = None;
                        self.status = format!("Deleted saved query {name}");
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            } else if let Some(name) = chosen {
                // Validate syntax once, without running a search (§41.1/§41.2).
                // The Search tab reparses the stored text itself on the next
                // frame; applying only restores that text and the mode.
                let found = self.saved_queries.iter().find(|query| query.name == name);
                match found.map(|query| search::validate(&query.text)) {
                    Some(Ok(())) => {
                        if let Some(query) = found {
                            self.search_query = query.text.clone();
                        }
                        self.search_mode = SearchMode::Query;
                        self.open_special(TabKind::Search);
                        self.status = format!("Applied saved query {name}");
                        self.error = None;
                        self.apply_query_open = false;
                        self.saved_query_rename = None;
                        self.saved_query_delete = None;
                    }
                    Some(Err(error)) => self.error = Some(error.to_string()),
                    None => self.error = Some("Saved query not found".to_string()),
                }
            } else if cancel || !open {
                self.apply_query_open = false;
                self.saved_query_rename = None;
                self.saved_query_delete = None;
                self.error = None;
            }
        }

        if self.save_workspace_open {
            let mut open = true;
            let mut cancel = false;
            let mut confirm = false;
            let count = self.tabs.len();
            let inline_error = self.error.clone();
            egui::Window::new("Save workspace")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 150.0))
                .show(&ctx, |ui| {
                    ui.colored_label(muted(), format!("{count} tabs will be saved"));
                    if let Some(error) = &inline_error {
                        ui.colored_label(rust(), error);
                    }
                    ui.horizontal(|ui| {
                        ui.label("name");
                        ui.text_edit_singleline(&mut self.workspace_name);
                    });
                    if ui.button("Save").clicked() || input_enter(&ctx) {
                        confirm = true;
                    }
                    if ui.button("Cancel").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            if confirm {
                let name = self.workspace_name.trim().to_string();
                if name.is_empty() {
                    self.error = Some("Workspace name is empty".to_string());
                } else {
                    let vault = self.root.to_string_lossy().to_string();
                    let descriptors = self
                        .tabs
                        .iter()
                        .map(|tab| (tab.kind.label().to_string(), tab.id.clone()))
                        .collect::<Vec<_>>();
                    let captured = workspace::capture(&vault, &name, &descriptors, self.active);
                    // §81: commit to memory only after the sidecar succeeds.
                    let updated = workspace::upsert(&self.workspaces, captured);
                    match workspace::save(&workspaces_path(), &updated) {
                        Ok(()) => {
                            self.workspaces = updated;
                            self.status = format!("Saved workspace {name}");
                            self.error = None;
                            self.save_workspace_open = false;
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                }
            } else if cancel || !open {
                self.save_workspace_open = false;
            }
        }

        if self.apply_workspace_open {
            let mut open = true;
            let mut cancel = false;
            let mut chosen: Option<String> = None;
            let mut rename_action: Option<(String, String)> = None;
            let mut delete_confirm: Option<String> = None;
            let mut clear_error = false;
            let vault = self.root.to_string_lossy().to_string();
            let rows: Vec<Workspace> = workspace::for_vault(&self.workspaces, &vault)
                .into_iter()
                .cloned()
                .collect();
            let inline_error = self.error.clone();
            let mut rename_target = self.workspace_rename.as_ref().map(|(old, _)| old.clone());
            let mut rename_draft = self
                .workspace_rename
                .as_ref()
                .map(|(_, draft)| draft.clone())
                .unwrap_or_default();
            let mut delete_target = self.workspace_delete.clone();
            egui::Window::new("Apply workspace")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(420.0, 320.0))
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 150.0))
                .show(&ctx, |ui| {
                    if let Some(error) = &inline_error {
                        ui.colored_label(rust(), error);
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("workspace_rows")
                        .show(ui, |ui| {
                            for row in &rows {
                                ui.horizontal(|ui| {
                                    if rename_target.is_none()
                                        && delete_target.is_none()
                                        && ui.button(&row.name).clicked()
                                    {
                                        chosen = Some(row.name.clone());
                                    }
                                    if rename_target.as_deref() != Some(row.name.as_str())
                                        && delete_target.is_none()
                                        && ui.small_button("Rename").clicked()
                                    {
                                        rename_target = Some(row.name.clone());
                                        rename_draft = row.name.clone();
                                        clear_error = true;
                                    }
                                    if rename_target.is_none()
                                        && ui.small_button("Delete").clicked()
                                    {
                                        delete_target = Some(row.name.clone());
                                        clear_error = true;
                                    }
                                });
                                ui.colored_label(muted(), format!("{} tabs", row.tabs.len()));
                                if rename_target.as_deref() == Some(row.name.as_str()) {
                                    ui.horizontal(|ui| {
                                        ui.text_edit_singleline(&mut rename_draft);
                                        if ui.small_button("OK").clicked() {
                                            rename_action =
                                                Some((row.name.clone(), rename_draft.clone()));
                                        }
                                        if ui.small_button("Cancel").clicked() {
                                            rename_target = None;
                                            clear_error = true;
                                        }
                                    });
                                }
                                if delete_target.as_deref() == Some(row.name.as_str()) {
                                    ui.horizontal(|ui| {
                                        ui.colored_label(rust(), "Delete this workspace?");
                                        if ui.small_button("Delete").clicked() {
                                            delete_confirm = Some(row.name.clone());
                                        }
                                        if ui.small_button("Cancel").clicked() {
                                            delete_target = None;
                                            clear_error = true;
                                        }
                                    });
                                }
                            }
                        });
                    if ui.button("Cancel").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            self.workspace_rename = rename_target.map(|old| (old, rename_draft));
            self.workspace_delete = delete_target;
            if clear_error {
                self.error = None;
            }

            if let Some((old, new)) = rename_action {
                match workspace::rename(&self.workspaces, &vault, &old, &new) {
                    Ok(updated) => match workspace::save(&workspaces_path(), &updated) {
                        Ok(()) => {
                            self.workspaces = updated;
                            self.workspace_rename = None;
                            self.status = format!("Renamed workspace {old} to {}", new.trim());
                            self.error = None;
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    },
                    Err(error) => self.error = Some(error.to_string()),
                }
            } else if let Some(name) = delete_confirm {
                let updated = workspace::remove(&self.workspaces, &vault, &name);
                match workspace::save(&workspaces_path(), &updated) {
                    Ok(()) => {
                        self.workspaces = updated;
                        self.workspace_delete = None;
                        self.status = format!("Deleted workspace {name}");
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            } else if let Some(name) = chosen {
                let mut applied = false;
                let selected = rows.iter().find(|row| row.name == name).cloned();
                if let Some(selected) = selected {
                    // §83: validate the whole descriptor list before any
                    // destructive step. A malformed kind must leave the
                    // current tabs intact instead of destroying them and
                    // asking the user to retry the same broken sidecar.
                    let mut malformed = false;
                    for descriptor in &selected.tabs {
                        if TabKind::parse(&descriptor.kind).is_none() {
                            self.error = Some(format!(
                                "Unknown workspace tab kind: {}",
                                descriptor.kind
                            ));
                            malformed = true;
                            break;
                        }
                    }
                    // §47: autosave may not have fired yet. Flush dirty note
                    // tabs before clearing so applying a workspace cannot
                    // silently drop edits. A tab still dirty after the flush
                    // (conflict or I/O error) aborts the apply and keeps the
                    // buffer for the user to resolve.
                    if !malformed {
                        for idx in 0..self.tabs.len() {
                            self.flush_tab(idx);
                        }
                        if self.tabs.iter().any(|tab| tab.dirty) {
                            // Keep the concrete I/O cause when flush_tab recorded
                            // one; only the conflict path has no error text.
                            let cause = self
                                .error
                                .clone()
                                .unwrap_or_else(|| "unsaved changes could not be saved".to_string());
                            self.error = Some(format!("Apply aborted: {cause}"));
                        } else {
                            // All descriptors are valid; a note that no longer
                            // exists is still skipped and the index clamped (§42.1).
                            self.tabs.clear();
                            self.active = 0;
                            let mut kept = Vec::with_capacity(selected.tabs.len());
                            for descriptor in &selected.tabs {
                                let before = self.tabs.len();
                                match TabKind::parse(&descriptor.kind) {
                                    Some(TabKind::Note) => {
                                        let id = NoteId::new(&descriptor.id);
                                        if self.index.notes.contains_key(&id) {
                                            self.open_note(&id);
                                        }
                                    }
                                    Some(kind) => self.open_special(kind),
                                    None => unreachable!("descriptors validated above (§83)"),
                                }
                                kept.push(self.tabs.len() > before);
                            }
                            // Remap the saved index over surviving tabs: open_note
                            // leaves active at the last opened tab (§42.1).
                            self.active = workspace::remap_active(&kept, selected.active);
                            let skipped = kept.iter().filter(|ok| !**ok).count();
                            self.status = if skipped > 0 {
                                format!("Applied workspace {name} · {skipped} tabs skipped")
                            } else {
                                format!("Applied workspace {name}")
                            };
                            self.error = None;
                            applied = true;
                        }
                    }
                } else {
                    self.error = Some("Workspace not found".to_string());
                }
                // Keep the manager open on abort/malformed data so the user can
                // retry; only a successful apply closes it (§48/§66).
                if applied {
                    self.apply_workspace_open = false;
                    self.workspace_rename = None;
                    self.workspace_delete = None;
                }
            } else if cancel || !open {
                self.apply_workspace_open = false;
                self.workspace_rename = None;
                self.workspace_delete = None;
                self.error = None;
            }
        }

        // §74: named Base views reuse the saved-query manager shape.
        if self.save_base_view_open {
            let mut open = true;
            let mut cancel = false;
            let mut confirm = false;
            let config = self.base_config.clone();
            let inline_error = self.error.clone();
            let preview_rows = bases::build(&self.index, &config).len();
            egui::Window::new("Save Base view")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 150.0))
                .show(&ctx, |ui| {
                    if let Some(error) = &inline_error {
                        ui.colored_label(rust(), error);
                    }
                    ui.colored_label(muted(), format!("{preview_rows} rows filtered"));
                    ui.horizontal(|ui| {
                        ui.label("name");
                        ui.text_edit_singleline(&mut self.base_view_name);
                    });
                    if ui.button("Save").clicked() || input_enter(&ctx) {
                        confirm = true;
                    }
                    if ui.button("Cancel").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            if confirm {
                let name = self.base_view_name.trim().to_string();
                match named_bases::upsert(&self.named_bases, &name, &config)
                    .and_then(|updated| {
                        named_bases::save(&named_bases_path(), &updated).map(|()| updated)
                    })
                {
                    Ok(updated) => {
                        self.named_bases = updated;
                        self.status = format!("Saved Base view {name}");
                        self.error = None;
                        self.save_base_view_open = false;
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            } else if cancel || !open {
                self.save_base_view_open = false;
            }
        }

        if self.apply_base_view_open {
            let mut open = true;
            let mut cancel = false;
            let mut chosen: Option<String> = None;
            let mut rename_action: Option<(String, String)> = None;
            let mut delete_confirm: Option<String> = None;
            let mut clear_error = false;
            let rows = self.named_bases.clone();
            let inline_error = self.error.clone();
            let mut rename_target = self.base_view_rename.as_ref().map(|(old, _)| old.clone());
            let mut rename_draft = self
                .base_view_rename
                .as_ref()
                .map(|(_, draft)| draft.clone())
                .unwrap_or_default();
            let mut delete_target = self.base_view_delete.clone();
            egui::Window::new("Saved Base views")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(420.0, 320.0))
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 150.0))
                .show(&ctx, |ui| {
                    if let Some(error) = &inline_error {
                        ui.colored_label(rust(), error);
                    }
                    if rows.is_empty() {
                        ui.colored_label(muted(), "No saved Base views yet");
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("named_base_rows")
                        .show(ui, |ui| {
                            for view in &rows {
                                ui.horizontal(|ui| {
                                    if ui.button(&view.name).clicked() {
                                        chosen = Some(view.name.clone());
                                    }
                                    if rename_target.as_deref() != Some(view.name.as_str())
                                        && delete_target.is_none()
                                        && ui.small_button("Rename").clicked()
                                    {
                                        rename_target = Some(view.name.clone());
                                        rename_draft = view.name.clone();
                                        clear_error = true;
                                    }
                                    if rename_target.is_none()
                                        && ui.small_button("Delete").clicked()
                                    {
                                        delete_target = Some(view.name.clone());
                                        clear_error = true;
                                    }
                                });
                                ui.colored_label(
                                    muted(),
                                    format!(
                                        "{} · {} · {}",
                                        view.config.filter_tag,
                                        view.config.filter_text,
                                        view.config.sort_by.label()
                                    ),
                                );
                                if rename_target.as_deref() == Some(view.name.as_str()) {
                                    ui.horizontal(|ui| {
                                        ui.text_edit_singleline(&mut rename_draft);
                                        if ui.small_button("OK").clicked() {
                                            rename_action =
                                                Some((view.name.clone(), rename_draft.clone()));
                                        }
                                        if ui.small_button("Cancel").clicked() {
                                            rename_target = None;
                                            clear_error = true;
                                        }
                                    });
                                }
                                if delete_target.as_deref() == Some(view.name.as_str()) {
                                    ui.horizontal(|ui| {
                                        ui.colored_label(rust(), "Delete this Base view?");
                                        if ui.small_button("Delete").clicked() {
                                            delete_confirm = Some(view.name.clone());
                                        }
                                        if ui.small_button("Cancel").clicked() {
                                            delete_target = None;
                                            clear_error = true;
                                        }
                                    });
                                }
                            }
                        });
                    if ui.button("Cancel").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            self.base_view_rename = rename_target.map(|old| (old, rename_draft));
            self.base_view_delete = delete_target;
            if clear_error {
                self.error = None;
            }

            if let Some((old, new)) = rename_action {
                match named_bases::rename(&self.named_bases, &old, &new)
                    .and_then(|updated| {
                        named_bases::save(&named_bases_path(), &updated).map(|()| updated)
                    })
                {
                    Ok(updated) => {
                        self.named_bases = updated;
                        self.base_view_rename = None;
                        self.status = format!("Renamed Base view {old} to {}", new.trim());
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            } else if let Some(name) = delete_confirm {
                let updated = named_bases::remove(&self.named_bases, &name);
                match named_bases::save(&named_bases_path(), &updated) {
                    Ok(()) => {
                        self.named_bases = updated;
                        self.base_view_delete = None;
                        self.status = format!("Deleted Base view {name}");
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            } else if let Some(name) = chosen {
                let found = self.named_bases.iter().find(|view| view.name == name);
                match found {
                    Some(view) => match bases::save(&base_path(), &view.config) {
                        Ok(()) => {
                            self.base_config = view.config.clone();
                            // §59 keeps the raw draft mirroring committed
                            // config; applying a view must refresh both.
                            self.base_sort_input = view.config.sort_field.clone();
                            self.base_column_input.clear();
                            self.base_edit = None;
                            self.open_special(TabKind::Base);
                            self.status = format!("Applied Base view {name}");
                            self.error = None;
                            self.apply_base_view_open = false;
                            self.base_view_rename = None;
                            self.base_view_delete = None;
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    },
                    None => self.error = Some("Base view not found".to_string()),
                }
            } else if cancel || !open {
                self.apply_base_view_open = false;
                self.base_view_rename = None;
                self.base_view_delete = None;
                self.error = None;
            }
        }

        if self.review_open {
            let mut open = true;
            let mut cancel = false;
            let mut chosen: Option<NoteId> = None;
            let rows = self.review_rows.clone();
            let (down, up, submit) = ctx.input(|input| {
                (
                    input.key_pressed(Key::ArrowDown),
                    input.key_pressed(Key::ArrowUp),
                    input.key_pressed(Key::Enter),
                )
            });
            let moved = (down || up) && !rows.is_empty();
            if !rows.is_empty() {
                self.review_selected = self.review_selected.min(rows.len() - 1);
                if down {
                    self.review_selected = (self.review_selected + 1) % rows.len();
                } else if up {
                    self.review_selected = (self.review_selected + rows.len() - 1) % rows.len();
                }
            }
            let mut selected_response = None;
            egui::Window::new("Review due notes")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(420.0, 320.0))
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 150.0))
                .show(&ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("review_rows")
                        .show(ui, |ui| {
                            for (index, id) in rows.iter().enumerate() {
                                let response = ui.selectable_label(index == self.review_selected, id.as_str());
                                if response.clicked() {
                                    chosen = Some(id.clone());
                                }
                                if index == self.review_selected {
                                    selected_response = Some(response);
                                }
                            }
                        });
                    if ui.button("Cancel").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            if moved {
                if let Some(response) = selected_response {
                    response.scroll_to_me(None);
                }
                ctx.input_mut(|input| {
                    input.events.retain(|event| {
                        !matches!(
                            event,
                            egui::Event::Key {
                                key: Key::ArrowUp | Key::ArrowDown | Key::Enter,
                                pressed: true,
                                ..
                            }
                        )
                    });
                });
            }
            if submit {
                if let Some(id) = rows.get(self.review_selected) {
                    chosen = Some(id.clone());
                }
                ctx.input_mut(|input| {
                    input.events.retain(|event| {
                        !matches!(
                            event,
                            egui::Event::Key {
                                key: Key::Enter,
                                pressed: true,
                                ..
                            }
                        )
                    });
                });
            }
            if let Some(id) = chosen {
                self.open_note(&id);
                self.review_open = false;
            } else if cancel || !open {
                self.review_open = false;
            }
        }

        if self.grade_open {
            let mut open = true;
            let mut cancel = false;
            let mut grade: Option<u8> = None;
            let inline_error = self.error.clone();
            egui::Window::new("Grade current note")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 150.0))
                .show(&ctx, |ui| {
                    if let Some(error) = &inline_error {
                        ui.colored_label(rust(), error);
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Again").clicked() {
                            grade = Some(AGAIN);
                        }
                        if ui.button("Hard").clicked() {
                            grade = Some(HARD);
                        }
                        if ui.button("Good").clicked() {
                            grade = Some(GOOD);
                        }
                        if ui.button("Easy").clicked() {
                            grade = Some(EASY);
                        }
                    });
                    if ui.button("Cancel").clicked() || keys.escape {
                        cancel = true;
                    }
                });
            if let Some(quality) = grade {
                if self.apply_grade(quality) {
                    self.grade_open = false;
                    self.error = None;
                }
            } else if cancel || !open {
                self.grade_open = false;
            }
        }

        // 9. Intercept renderer links. Nexora never opens an external browser.
        let mut link_navigation = None;
        let mut external_blocked = false;
        ctx.output_mut(|output| {
            let mut kept = Vec::with_capacity(output.commands.len());
            for command in output.commands.drain(..) {
                match command {
                    egui::OutputCommand::OpenUrl(open) => {
                        if let Some(encoded) = open.url.strip_prefix("nexora://") {
                            // Split before decoding: a literal `#` inside a
                            // note id is `%23` on the wire and must not become
                            // an anchor (§38.2).
                            let (note, anchor) = match encoded.split_once('#') {
                                Some((note, anchor)) if !anchor.is_empty() => {
                                    (decode_target(note), Some(decode_target(anchor)))
                                }
                                _ => (decode_target(encoded), None),
                            };
                            link_navigation = Some((NoteId::new(note), anchor));
                        } else {
                            external_blocked = true;
                        }
                    }
                    other => kept.push(other),
                }
            }
            output.commands = kept;
        });
        if external_blocked {
            self.status = "External links are disabled in Nexora".to_string();
        }
        if let Some((target, anchor)) = link_navigation {
            if self.index.notes.contains_key(&target) {
                self.selected_broken = None;
                self.open_note(&target);
                if let Some(anchor) = anchor {
                    self.jump_to_anchor(&target, &anchor);
                }
            } else {
                self.selected_broken = Some(target.clone());
                self.status = format!("Broken link: {target} (Ctrl+P → Create note at target)");
            }
        }
        // §86: recovery is reported once per frame, after every other status
        // write (save/create/capture/etc.), so the notice is never lost.
        if self.recovery_notice {
            self.status = "Vault index restored".to_string();
            self.recovery_notice = false;
        }
    }
}

/// Repaint the shell from one authored preset (ARCHITECTURE.md §35). Called at
/// startup and on every theme cycle; there is deliberately no colour picker.
fn apply_style(ctx: &egui::Context, preset: ThemePreset) {
    set_palette(preset);
    let theme = if preset.is_dark() {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_theme(if preset.is_dark() {
        egui::ThemePreference::Dark
    } else {
        egui::ThemePreference::Light
    });
    let mut visuals = ctx.style_of(theme).visuals.clone();
    visuals.dark_mode = preset.is_dark();
    // DESIGN v2 (§85): graphite shells, lavender accent, pill-like chrome.
    visuals.panel_fill = panel();
    visuals.window_fill = surface();
    visuals.window_stroke = Stroke::new(1.0, line());
    // egui_commonmark paints code blocks with `extreme_bg_color`, not
    // `code_bg_color` (elements.rs:229); both come from the same table.
    visuals.extreme_bg_color = raised();
    visuals.text_edit_bg_color = Some(surface());
    visuals.code_bg_color = raised();
    visuals.hyperlink_color = link();
    visuals.warn_fg_color = amber();
    visuals.error_fg_color = rust();
    visuals.override_text_color = Some(text());
    visuals.widgets.noninteractive.bg_fill = panel();
    visuals.widgets.noninteractive.bg_stroke.color = line();
    visuals.widgets.noninteractive.fg_stroke.color = muted();
    visuals.widgets.inactive.bg_fill = surface();
    visuals.widgets.inactive.fg_stroke.color = text();
    visuals.widgets.hovered.bg_fill = tag_hover();
    visuals.widgets.hovered.fg_stroke.color = text();
    visuals.widgets.active.bg_fill = accent_dim();
    visuals.widgets.active.fg_stroke.color = text();
    visuals.widgets.open.bg_fill = tag();
    visuals.selection.bg_fill = accent_dim();
    // §85: visible focus ring, 2 px normally and 3 px in HighContrast.
    // egui paints focus via `active.bg_stroke`; selection stroke is the
    // companion for selected text/rows.
    let focus_width = if preset == ThemePreset::HighContrast { 3.0 } else { 2.0 };
    visuals.widgets.active.bg_stroke = Stroke::new(focus_width, accent());
    visuals.selection.stroke = Stroke::new(focus_width, accent());
    // 8 px corners everywhere; soft shadow only on floating layers.
    let radius = egui::CornerRadius::same(8);
    for widgets in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widgets.corner_radius = radius;
    }
    visuals.window_corner_radius = radius;
    visuals.menu_corner_radius = radius;
    visuals.window_shadow = egui::Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: egui::Color32::from_black_alpha(80),
    };

    let mut style = (*ctx.style_of(theme)).clone();
    style.visuals = visuals;
    // DESIGN v2 type scale: H1 30 / body 14.5 / button 13.5 / small 12 /
    // mono 13. egui_commonmark still interpolates H2/H3 from Heading+Body.
    style.text_styles.insert(TextStyle::Heading, FontId::proportional(30.0));
    style.text_styles.insert(TextStyle::Body, FontId::proportional(14.5));
    style.text_styles.insert(TextStyle::Button, FontId::proportional(13.5));
    style.text_styles.insert(TextStyle::Small, FontId::proportional(12.0));
    style.text_styles.insert(TextStyle::Monospace, FontId::monospace(13.0));
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(9.0, 5.0);
    style.spacing.window_margin = Margin::same(12);
    ctx.set_style_of(theme, style);
}

/// Embed the two OFL font files once at startup (§85). No runtime network.
fn install_fonts(cc: &eframe::CreationContext<'_>) {
    let inter = FontInsert::new(
        "Inter",
        FontData::from_static(include_bytes!("../assets/fonts/InterVariable.ttf")),
        vec![InsertFontFamily {
            family: FontFamily::Proportional,
            priority: FontPriority::Highest,
        }],
    );
    let mono = FontInsert::new(
        "JetBrains Mono",
        FontData::from_static(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")),
        vec![InsertFontFamily {
            family: FontFamily::Monospace,
            priority: FontPriority::Highest,
        }],
    );
    cc.egui_ctx.add_font(inter);
    cc.egui_ctx.add_font(mono);
}

/// Very faint vertical gradient on the active panel only (§85). Uses a
/// coloured mesh because egui 0.37 has no gradient fill primitive.
fn paint_gradient(ui: &mut egui::Ui, rect: egui::Rect) {
    let (a, b) = (palette().bg, palette().tag);
    let mix = |t: f32| -> Color32 {
        let channel = |i: usize| (a[i] as f32 * (1.0 - t) + b[i] as f32 * t) as u8;
        Color32::from_rgb(channel(0), channel(1), channel(2))
    };
    let top = mix(0.35);
    let bottom = mix(0.0);
    let mut mesh = egui::Mesh::default();
    mesh.vertices.push(Vertex::untextured(rect.left_top(), top));
    mesh.vertices.push(Vertex::untextured(rect.right_top(), top));
    mesh.vertices.push(Vertex::untextured(rect.right_bottom(), bottom));
    mesh.vertices.push(Vertex::untextured(rect.left_bottom(), bottom));
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    ui.painter().add(egui::Shape::mesh(mesh));
}

#[derive(Clone, Copy)]
enum RailIcon {
    Today,
    Health,
    Graph,
    Canvas,
    Base,
    Vectors,
    Capture,
}

/// Linear icon drawn by hand (§85): no icon pack, no emoji, no new dependency.
fn rail_button(ui: &mut egui::Ui, icon: RailIcon, label: &str, active: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    let color = if active { accent() } else { muted() };
    let stroke = Stroke::new(1.6, color);
    let painter = ui.painter();
    let center = rect.center();
    match icon {
        RailIcon::Today => {
            let r = rect.shrink(6.0);
            painter.rect_stroke(r, egui::CornerRadius::same(3), stroke, egui::StrokeKind::Outside);
            painter.line_segment([egui::pos2(r.left(), r.top() + 5.0), egui::pos2(r.right(), r.top() + 5.0)], stroke);
        }
        RailIcon::Health => {
            painter.line_segment([center - Vec2::new(0.0, 9.0), center + Vec2::new(0.0, 9.0)], stroke);
            painter.line_segment([center - Vec2::new(9.0, 0.0), center + Vec2::new(9.0, 0.0)], stroke);
        }
        RailIcon::Graph => {
            let points = [
                center + Vec2::new(0.0, -9.0),
                center + Vec2::new(-8.0, 7.0),
                center + Vec2::new(8.0, 7.0),
            ];
            for i in 0..3 {
                painter.line_segment([points[i], points[(i + 1) % 3]], stroke);
            }
            for point in points {
                painter.circle_filled(point, 2.0, color);
            }
        }
        RailIcon::Canvas => {
            painter.rect_stroke(rect.shrink(6.0), egui::CornerRadius::same(3), stroke, egui::StrokeKind::Outside);
            painter.circle_filled(center, 2.0, color);
        }
        RailIcon::Base => {
            let r = rect.shrink(6.0);
            let mid_x = (r.left() + r.right()) / 2.0;
            let mid_y = (r.top() + r.bottom()) / 2.0;
            painter.rect_stroke(r, egui::CornerRadius::same(2), stroke, egui::StrokeKind::Outside);
            painter.line_segment([egui::pos2(mid_x, r.top()), egui::pos2(mid_x, r.bottom())], stroke);
            painter.line_segment([egui::pos2(r.left(), mid_y), egui::pos2(r.right(), mid_y)], stroke);
        }
        RailIcon::Vectors => {
            for dx in [-9.0, 0.0, 9.0] {
                painter.line_segment([center + Vec2::new(dx, -8.0), center], stroke);
            }
            painter.circle_filled(center, 2.5, color);
        }
        RailIcon::Capture => {
            painter.circle_stroke(center, 9.0, stroke);
            painter.line_segment([center - Vec2::new(4.0, 0.0), center + Vec2::new(4.0, 0.0)], stroke);
            painter.line_segment([center - Vec2::new(0.0, 4.0), center + Vec2::new(0.0, 4.0)], stroke);
        }
    }
    response.on_hover_text(label).clicked()
}

/// Obsidian-style callout accents (§85). Plain letters, no emoji icons.
fn callout_alerts() -> AlertBundle {
    let alert = |identifier: &str, rendered: &str, icon: char, color: Color32| Alert {
        accent_color: color,
        icon,
        identifier: identifier.to_string(),
        identifier_rendered: rendered.to_string(),
    };
    AlertBundle::from_alerts(vec![
        alert("NOTE", "Note", 'i', info_fg()),
        alert("TIP", "Tip", '+', sage()),
        alert("IMPORTANT", "Important", '*', accent()),
        alert("WARNING", "Warning", '!', amber()),
        alert("CAUTION", "Caution", 'x', rust()),
    ])
}

fn hover_preview(_ui: &mut egui::Ui, response: egui::Response, data: Option<Preview>) {
    response.on_hover_ui(move |ui| {
        if let Some(preview) = data {
            ui.set_max_width(320.0);
            ui.colored_label(accent(), &preview.title);
            if preview.excerpt.is_empty() {
                ui.colored_label(muted(), "empty note");
            } else {
                ui.label(&preview.excerpt);
            }
        }
    });
}

fn input_enter(ctx: &egui::Context) -> bool {
    ctx.input(|input| input.key_pressed(Key::Enter))
}

/// Single owner of editor cursor restoration after a core text edit (§75 review).
fn set_editor_cursor(ctx: &egui::Context, editor_id: egui::Id, cursor: usize) {
    if let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) {
        state.cursor.set_char_range(Some(
            egui::text::CCursorRange::one(egui::text::CCursor::new(cursor)),
        ));
        egui::TextEdit::store_state(ctx, editor_id, state);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1120.0, 680.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Nexora",
        options,
        Box::new(|cc| {
            install_fonts(cc);
            let app = NexoraApp::new();
            apply_style(&cc.egui_ctx, app.theme);
            Ok(Box::new(app))
        }),
    )
}

