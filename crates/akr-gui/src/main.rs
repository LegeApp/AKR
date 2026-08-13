#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#![allow(missing_docs)]

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::Instant;

use akr_gui::model::{
    AcceptanceCheck, Diagnostic, GitMetadata, LoadPhase, Record, Relation, ReviewCounts,
    ReviewSnapshot, SnapshotError, WorkspaceLoader,
};
use akr_gui::render::Canvas;
use akr_gui::ui::{AppModel, Panel, TreeMode};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

const BG: u32 = 0x0010_1720;
const SURFACE: u32 = 0x001a_2633;
const RAISED: u32 = 0x0022_3444;
const LINE: u32 = 0x0032_4658;
const TEXT: u32 = 0x00d8_e3ed;
const MUTED: u32 = 0x008d_a4b8;
const ACCENT: u32 = 0x0056_b6d9;
const SELECTED: u32 = 0x0033_5870;
const WARNING: u32 = 0x00ed_b55f;
const THUMB: u32 = 0x0047_6178;

/// Minimum readable glyph magnification; the 8x8 bitmap font is unreadable at
/// 1x on any modern display, so the shell never renders below 2x.
const MIN_SCALE: i32 = 2;
const MAX_SCALE: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Rect {
    fn contains(self, x: f64, y: f64) -> bool {
        let (x, y) = (x as i32, y as i32);
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
    fn inset(self, amount: i32) -> Self {
        Self {
            x: self.x + amount,
            y: self.y + amount,
            w: (self.w - amount * 2).max(0),
            h: (self.h - amount * 2).max(0),
        }
    }
}

/// Text metrics for the current glyph scale. Every layout number in the shell
/// is derived from these so the window, the panes and the type all resize
/// together instead of drifting apart.
#[derive(Debug, Clone, Copy)]
struct Metrics {
    scale: i32,
    cw: i32,
    row: i32,
    pad: i32,
    bar: i32,
}

impl Metrics {
    fn new(scale: i32) -> Self {
        let cw = 8 * scale;
        let row = 8 * scale + 4 * scale;
        let pad = 4 * scale;
        Self {
            scale,
            cw,
            row,
            pad,
            bar: row + pad * 2,
        }
    }
    fn width_of(self, text: &str) -> i32 {
        text.chars().count() as i32 * self.cw
    }
}

/// Everything the frame needs that is not ledger state.
#[derive(Debug, Clone, Copy)]
struct ViewState {
    tree_scroll: i32,
    panel_scroll: i32,
    /// Left pane width as a fraction of the window, so panes keep their
    /// proportions when the window is resized.
    split: f64,
    filter_editing: bool,
    /// True while the "open workspace" prompt owns the keyboard.
    path_editing: bool,
    scale: i32,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            tree_scroll: 0,
            panel_scroll: 0,
            split: 0.30,
            filter_editing: false,
            path_editing: false,
            scale: MIN_SCALE,
        }
    }
}

struct Layout {
    metrics: Metrics,
    header: Rect,
    tabs: Vec<(Rect, usize)>,
    open_button: Option<Rect>,
    filter_bar: Rect,
    filter_box: Rect,
    buttons: Vec<(Rect, Panel, &'static str)>,
    tree_header: Rect,
    mode_button: Rect,
    tree_list: Rect,
    splitter: Rect,
    panel: Rect,
    footer: Rect,
}

/// Panel switcher entries as (panel, full label, short label). The short label
/// is used when the window is too narrow for the full one at the current zoom,
/// and the switcher is dropped entirely rather than crowd out the filter box.
const PANEL_BUTTONS: [(Panel, &str, &str); 4] = [
    (Panel::Dashboard, "Dashboard", "D"),
    (Panel::Inspector, "Detail", "I"),
    (Panel::Relations, "Relations", "L"),
    (Panel::Git, "Git", "G"),
];

fn layout(width: i32, height: i32, model: &AppModel, view: ViewState) -> Layout {
    let metrics = Metrics::new(view.scale);
    let Metrics { cw, pad, bar, .. } = metrics;

    let header = Rect {
        x: 0,
        y: 0,
        w: width,
        h: bar,
    };
    let mut tabs = Vec::new();
    let mut x = pad + metrics.width_of("AKR REVIEW") + cw * 2;
    for (index, tab) in model.tabs.iter().enumerate() {
        let label = workspace_label(tab.workspace.as_path());
        let w = metrics.width_of(&label) + pad * 2;
        if x + w > width - pad {
            break;
        }
        tabs.push((
            Rect {
                x,
                y: pad / 2,
                w,
                h: bar - pad,
            },
            index,
        ));
        x += w + pad;
    }
    let open_w = metrics.width_of("+ Open") + pad * 2;
    let open_button = (x + open_w <= width - pad).then_some(Rect {
        x,
        y: pad / 2,
        w: open_w,
        h: bar - pad,
    });

    let filter_bar = Rect {
        x: 0,
        y: bar,
        w: width,
        h: bar,
    };
    // The filter box always keeps a usable minimum; the switcher degrades to
    // short labels and then disappears when the window cannot fit it.
    let filter_min = cw * 10;
    let switcher_width = |short: bool| {
        PANEL_BUTTONS
            .iter()
            .map(|(_, long, brief)| metrics.width_of(if short { brief } else { long }) + pad * 3)
            .sum::<i32>()
    };
    let short = width - pad * 2 - switcher_width(false) < filter_min;
    let mut buttons = Vec::new();
    let mut right = width - pad;
    if width - pad * 2 - switcher_width(short) >= filter_min {
        for (panel, long, brief) in PANEL_BUTTONS.iter().rev() {
            let label = if short { *brief } else { *long };
            let w = metrics.width_of(label) + pad * 2;
            right -= w;
            buttons.push((
                Rect {
                    x: right,
                    y: filter_bar.y + pad / 2,
                    w,
                    h: bar - pad,
                },
                *panel,
                label,
            ));
            right -= pad;
        }
        buttons.reverse();
    }
    let filter_box = Rect {
        x: pad,
        y: filter_bar.y + pad / 2,
        w: (right - pad * 2).max(cw * 2),
        h: bar - pad,
    };

    let footer = Rect {
        x: 0,
        y: height - bar,
        w: width,
        h: bar,
    };
    let content_y = filter_bar.y + filter_bar.h;
    let content_h = (footer.y - content_y).max(0);

    let handle = (pad / 2).max(3);
    // Minimums are expressed in glyphs but capped as a share of the window, so
    // a small window at a large zoom still yields two non-empty panes.
    let min_left = (cw * 14).min(width / 4).max(0);
    let min_right = (cw * 20).min(width / 2);
    let max_left = (width - min_right - handle).max(0);
    let split_x = ((width as f64 * view.split) as i32).clamp(min_left.min(max_left), max_left);

    let tree_header = Rect {
        x: 0,
        y: content_y,
        w: split_x,
        h: bar,
    };
    let mode_label = mode_label(model.mode);
    let mode_w = metrics.width_of(mode_label) + pad * 2;
    let mode_button = Rect {
        x: (split_x - mode_w - pad).max(pad),
        y: content_y + pad / 2,
        w: mode_w.min(split_x - pad * 2),
        h: bar - pad,
    };
    let tree_list = Rect {
        x: 0,
        y: content_y + bar,
        w: split_x,
        h: (content_h - bar).max(0),
    };
    let splitter = Rect {
        x: split_x,
        y: content_y,
        w: handle,
        h: content_h,
    };
    let panel = Rect {
        x: split_x + handle,
        y: content_y,
        w: (width - split_x - handle).max(0),
        h: content_h,
    };

    Layout {
        metrics,
        header,
        tabs,
        open_button,
        filter_bar,
        filter_box,
        buttons,
        tree_header,
        mode_button,
        tree_list,
        splitter,
        panel,
        footer,
    }
}

/// Resolves a typed or dropped path to the workspace to open.
///
/// A file resolves to its directory, and any directory inside a ledger resolves
/// to the nearest ancestor holding `.akr`, so dropping a source file on the
/// window opens that file's project rather than a directory with no records.
/// A directory with no `.akr` anywhere above it is still opened — the load will
/// report the real diagnostic — but a path that does not exist is refused here.
fn workspace_root(requested: &Path) -> Result<PathBuf, String> {
    let path = requested
        .canonicalize()
        .map_err(|error| format!("{}: {error}", requested.display()))?;
    let start = if path.is_dir() {
        path.as_path()
    } else {
        path.parent().unwrap_or(path.as_path())
    };
    let resolved = start
        .ancestors()
        .find(|candidate| candidate.join(".akr").is_dir())
        .unwrap_or(start);
    // Windows canonicalisation yields a \\?\ prefix that reads badly in the
    // title bar and in tab chips; strip it back to a conventional path.
    let text = resolved.to_string_lossy();
    Ok(match text.strip_prefix(r"\\?\") {
        Some(trimmed) => PathBuf::from(trimmed),
        None => resolved.to_path_buf(),
    })
}

fn workspace_label(workspace: &Path) -> String {
    workspace
        .file_name()
        .and_then(|part| part.to_str())
        .unwrap_or("workspace")
        .to_owned()
}

fn mode_label(mode: TreeMode) -> &'static str {
    match mode {
        TreeMode::Planning => "Planning",
        TreeMode::Knowledge => "Knowledge",
    }
}

/// A clipped, scrollable column of text rows. Panels paint into one of these
/// instead of computing absolute pixel offsets, which is what previously made
/// every panel overflow the window.
struct Pane<'a> {
    canvas: &'a mut Canvas,
    rect: Rect,
    metrics: Metrics,
    scroll: i32,
    cursor: i32,
}

impl<'a> Pane<'a> {
    fn new(canvas: &'a mut Canvas, rect: Rect, metrics: Metrics, scroll: i32) -> Self {
        Self {
            canvas,
            rect: rect.inset(metrics.pad),
            metrics,
            scroll,
            cursor: 0,
        }
    }
    fn row_top(&self) -> i32 {
        self.rect.y + self.cursor - self.scroll
    }
    fn visible(&self) -> bool {
        let y = self.row_top();
        y + self.metrics.row > self.rect.y && y < self.rect.y + self.rect.h
    }
    fn advance(&mut self) {
        self.cursor += self.metrics.row;
    }
    fn line(&mut self, text: &str, color: u32) {
        self.indented(0, text, color);
    }
    fn indented(&mut self, indent: i32, text: &str, color: u32) {
        if self.visible() {
            let y = self.row_top();
            self.canvas
                .text_clipped(self.rect.x + indent, y, self.rect.w - indent, text, color);
        }
        self.advance();
    }
    fn selectable(&mut self, indent: i32, text: &str, selected: bool) {
        if self.visible() {
            let y = self.row_top();
            if selected {
                self.canvas.rect(
                    self.rect.x - self.metrics.pad / 2,
                    y - self.metrics.pad / 2,
                    self.rect.w + self.metrics.pad,
                    self.metrics.row,
                    SELECTED,
                );
            }
            self.canvas.text_clipped(
                self.rect.x + indent,
                y,
                self.rect.w - indent,
                text,
                if selected { TEXT } else { MUTED },
            );
        }
        self.advance();
    }
    fn heading(&mut self, text: &str) {
        self.gap(1);
        self.line(text, ACCENT);
    }
    /// A determinate bar filling `done` of `total`, one row tall.
    fn progress(&mut self, done: usize, total: usize) {
        if self.visible() {
            let y = self.row_top();
            let height = (self.metrics.scale * 2).max(4);
            let width = self.rect.w.min(self.metrics.cw * 40);
            let top = y + (self.metrics.row - height) / 2;
            self.canvas.rect(self.rect.x, top, width, height, RAISED);
            self.canvas.border(self.rect.x, top, width, height, LINE);
            let filled = if total == 0 {
                0
            } else {
                (width as i64 * done.min(total) as i64 / total as i64) as i32
            };
            self.canvas.rect(self.rect.x, top, filled, height, ACCENT);
        }
        self.advance();
    }
    fn gap(&mut self, rows: i32) {
        self.cursor += self.metrics.row * rows;
    }
    fn paragraph(&mut self, text: &str, color: u32) {
        let columns = (self.rect.w / self.metrics.cw).max(8) as usize;
        for block in text.split('\n') {
            if block.trim().is_empty() {
                self.gap(1);
                continue;
            }
            for line in wrap(block, columns) {
                self.line(&line, color);
            }
        }
    }
    fn content_height(&self) -> i32 {
        self.cursor + self.metrics.pad
    }
}

/// Draws a proportional scrollbar on the right edge of `rect`.
fn scrollbar(canvas: &mut Canvas, rect: Rect, metrics: Metrics, scroll: i32, content: i32) {
    if content <= rect.h || rect.h <= 0 {
        return;
    }
    let width = (metrics.scale * 2).max(4);
    let track = Rect {
        x: rect.x + rect.w - width - 1,
        y: rect.y,
        w: width,
        h: rect.h,
    };
    canvas.rect(track.x, track.y, track.w, track.h, SURFACE);
    let thumb_h = ((rect.h as i64 * rect.h as i64 / content as i64) as i32).max(metrics.row);
    let span = (content - rect.h).max(1);
    let offset = ((rect.h - thumb_h) as i64 * scroll.clamp(0, span) as i64 / span as i64) as i32;
    canvas.rect(track.x, track.y + offset, track.w, thumb_h, THUMB);
}

fn button(canvas: &mut Canvas, rect: Rect, metrics: Metrics, label: &str, active: bool) {
    canvas.rect(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        if active { SELECTED } else { RAISED },
    );
    canvas.border(rect.x, rect.y, rect.w, rect.h, LINE);
    canvas.text_clipped(
        rect.x + metrics.pad,
        rect.y + (rect.h - metrics.cw).max(0) / 2,
        rect.w - metrics.pad * 2,
        label,
        if active { TEXT } else { MUTED },
    );
}

#[derive(Clone)]
struct AkrLoader;
impl WorkspaceLoader for AkrLoader {
    fn load_reporting(
        &self,
        workspace: &Path,
        progress: &(dyn Fn(LoadPhase) + Sync),
    ) -> Result<ReviewSnapshot, SnapshotError> {
        use akr_cli::review_snapshot::ReviewPhase;
        let source = akr_cli::review_snapshot::ReviewSnapshot::load_reporting(
            workspace,
            akr_cli::review_snapshot::ReviewOptions::default(),
            &|phase| {
                progress(LoadPhase {
                    label: phase.label().to_owned(),
                    step: phase.step(),
                    total: ReviewPhase::ALL.len(),
                });
            },
        )
        .map_err(|error| SnapshotError {
            message: error.to_string(),
        })?;
        Ok(ReviewSnapshot {
            workspace: source.workspace,
            project: source.project,
            source_graph: source.source_graph,
            head: source.head,
            counts: ReviewCounts {
                records: source.counts.records,
                revisions: source.counts.revisions,
                stale: source.counts.stale,
                at_risk: source.counts.at_risk,
                diagnostics: source.counts.diagnostics,
                live_planning: source.counts.live_planning,
                open_questions: source.counts.open_questions,
            },
            diagnostics: source
                .diagnostics
                .into_iter()
                .map(|diagnostic| Diagnostic {
                    severity: diagnostic.severity,
                    code: diagnostic.code,
                    message: diagnostic.message,
                })
                .collect(),
            records: source
                .records
                .into_iter()
                .map(|record| {
                    let body = record
                        .slots
                        .iter()
                        .map(|field| format!("{}: {}", field.name, field.value))
                        .chain(
                            record
                                .claims
                                .iter()
                                .map(|claim| format!("claim #{}: {}", claim.anchor, claim.text)),
                        )
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    let relations = record
                        .relations
                        .iter()
                        .filter(|relation| relation.direction == "outbound")
                        .map(|relation| Relation {
                            kind: relation.relation.clone(),
                            target: relation
                                .record
                                .split_once('/')
                                .map_or_else(|| relation.record.clone(), |(key, _)| key.to_owned()),
                        })
                        .collect();
                    let provenance = record
                        .provenance
                        .iter()
                        .map(|source| {
                            let locator = source
                                .document
                                .as_deref()
                                .or(source.path.as_deref())
                                .or(source.url.as_deref())
                                .unwrap_or("unlocated");
                            match &source.range {
                                Some(range) => format!("{} {} {range}", source.kind, locator),
                                None => format!("{} {}", source.kind, locator),
                            }
                        })
                        .collect();
                    Record {
                        key: record.key,
                        title: record.title,
                        kind: record.kind,
                        state: record.state,
                        revision: record.revision,
                        body: if body.is_empty() { record.body } else { body },
                        freshness: record.freshness.status.clone(),
                        plan_of_record: !record.plan_for.is_empty(),
                        relations,
                        acceptance: record
                            .acceptance
                            .into_iter()
                            .map(|check| AcceptanceCheck {
                                id: check.id,
                                statement: check.statement,
                                verdict: check.verdict,
                            })
                            .collect(),
                        provenance,
                        history: record
                            .history
                            .into_iter()
                            .map(|revision| revision.id)
                            .collect(),
                        git: GitMetadata {
                            defined_at: record.defined_at,
                            observed_at: record.observed_at,
                            stale_cause: record.freshness.cause,
                        },
                    }
                })
                .collect(),
        })
    }
}

/// What a worker thread sends back: stage boundaries as they pass, then the
/// finished snapshot. Both carry the generation, so a superseded load's
/// messages are discarded rather than overwriting a newer one.
enum LoadMessage {
    Phase {
        tab: usize,
        generation: u64,
        phase: LoadPhase,
    },
    Done {
        tab: usize,
        generation: u64,
        result: Box<Result<ReviewSnapshot, SnapshotError>>,
    },
}

struct NativeApp {
    window: Option<Arc<Window>>,
    context: Option<Context<Arc<Window>>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    canvas: Canvas,
    model: AppModel,
    view: ViewState,
    pointer: (f64, f64),
    dragging_split: bool,
    control_held: bool,
    path_input: String,
    status: Option<String>,
    load_started: Option<Instant>,
    /// Frame counter, so the loading marker animates without a wall clock.
    tick: u64,
    tree_content: i32,
    panel_content: i32,
    loads: mpsc::Receiver<LoadMessage>,
    sender: mpsc::Sender<LoadMessage>,
    loader: Arc<dyn WorkspaceLoader>,
}

impl NativeApp {
    fn new(workspaces: Vec<PathBuf>) -> Self {
        let (sender, loads) = mpsc::channel();
        let mut model = AppModel::default();
        for workspace in workspaces {
            model.add_tab(workspace);
        }
        if model.tabs.is_empty() {
            model.add_tab(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        }
        Self {
            window: None,
            context: None,
            surface: None,
            canvas: Canvas::new(1, 1),
            model,
            view: ViewState::default(),
            pointer: (0.0, 0.0),
            dragging_split: false,
            control_held: false,
            path_input: String::new(),
            status: None,
            load_started: None,
            tick: 0,
            tree_content: 0,
            panel_content: 0,
            loads,
            sender,
            loader: Arc::new(AkrLoader),
        }
    }
    fn request_load(&mut self, tab: usize) {
        let Some(workspace) = self.model.tabs.get_mut(tab).map(|tab| {
            tab.load_generation += 1;
            tab.error = None;
            tab.phase = None;
            tab.workspace.clone()
        }) else {
            return;
        };
        let generation = self.model.tabs[tab].load_generation;
        let sender = self.sender.clone();
        let loader = self.loader.clone();
        self.load_started = Some(Instant::now());
        std::thread::spawn(move || {
            let progress = sender.clone();
            let result = loader.load_reporting(&workspace, &|phase| {
                let _ = progress.send(LoadMessage::Phase {
                    tab,
                    generation,
                    phase,
                });
            });
            let _ = sender.send(LoadMessage::Done {
                tab,
                generation,
                result: Box::new(result),
            });
        });
    }
    fn request_active_load(&mut self) {
        if let Some(tab) = self.model.active_tab {
            self.request_load(tab);
        }
    }
    /// Opens a workspace in a tab, reusing the tab if it is already open.
    fn open_workspace(&mut self, requested: &Path) {
        let path = match workspace_root(requested) {
            Ok(path) => path,
            Err(message) => {
                self.status = Some(message);
                return;
            }
        };
        self.view.tree_scroll = 0;
        self.view.panel_scroll = 0;
        if let Some(index) = self.model.tabs.iter().position(|tab| tab.workspace == path) {
            self.model.active_tab = Some(index);
            self.status = Some(format!("already open: {}", path.display()));
            self.sync_title();
            return;
        }
        let index = self.model.add_tab(path.clone());
        self.status = Some(format!("opened {}", path.display()));
        self.sync_title();
        self.request_load(index);
    }
    /// Starts the open prompt, seeded with the active workspace's parent so the
    /// common case — a sibling project — is a short edit rather than a retype.
    fn begin_open_prompt(&mut self) {
        self.path_input = self
            .model
            .active_tab
            .and_then(|index| self.model.tabs.get(index))
            .and_then(|tab| tab.workspace.parent())
            .map(|parent| {
                let text = parent.to_string_lossy().into_owned();
                if text.ends_with(['/', '\\']) {
                    text
                } else {
                    format!("{text}{}", std::path::MAIN_SEPARATOR)
                }
            })
            .unwrap_or_default();
        self.view.path_editing = true;
        self.view.filter_editing = false;
        self.status = None;
    }
    fn submit_open_prompt(&mut self) {
        self.view.path_editing = false;
        let typed = self.path_input.trim().trim_matches('"').to_owned();
        if typed.is_empty() {
            return;
        }
        self.open_workspace(Path::new(&typed));
    }
    fn close_active_workspace(&mut self) {
        let Some(index) = self.model.active_tab else {
            return;
        };
        let label = workspace_label(self.model.tabs[index].workspace.as_path());
        self.model.close_active_tab();
        self.view.tree_scroll = 0;
        self.view.panel_scroll = 0;
        self.status = Some(format!("closed {label}"));
        self.sync_title();
    }
    /// Mirrors the active workspace into the OS title bar, so the full path is
    /// visible even when the tab chip is truncated.
    fn sync_title(&self) {
        let Some(window) = &self.window else {
            return;
        };
        match self
            .model
            .active_tab
            .and_then(|index| self.model.tabs.get(index))
        {
            Some(tab) => window.set_title(&format!("AKR Review — {}", tab.workspace.display())),
            None => window.set_title("AKR Review"),
        }
    }
    fn poll_loads(&mut self) {
        while let Ok(message) = self.loads.try_recv() {
            let (index, generation) = match &message {
                LoadMessage::Phase {
                    tab, generation, ..
                }
                | LoadMessage::Done {
                    tab, generation, ..
                } => (*tab, *generation),
            };
            // A tab that was closed, or a load that a newer one superseded.
            if self
                .model
                .tabs
                .get(index)
                .is_none_or(|tab| tab.load_generation != generation)
            {
                continue;
            }
            match message {
                LoadMessage::Phase { phase, .. } => {
                    if let Some(tab) = self.model.tabs.get_mut(index) {
                        tab.phase = Some(phase);
                    }
                }
                LoadMessage::Done { result, .. } => {
                    let elapsed = self.load_started.take().map(|start| start.elapsed());
                    match *result {
                        Ok(snapshot) => {
                            let records = snapshot.records.len();
                            self.model.set_snapshot(index, snapshot);
                            self.status = Some(match elapsed {
                                Some(elapsed) => format!(
                                    "loaded {records} records in {:.1}s",
                                    elapsed.as_secs_f32()
                                ),
                                None => format!("loaded {records} records"),
                            });
                        }
                        Err(error) => {
                            self.model.fail_load(index, error.message);
                            self.status = Some("load failed".to_owned());
                        }
                    }
                    if let Some(tab) = self.model.tabs.get_mut(index) {
                        tab.phase = None;
                    }
                    self.sync_title();
                }
            }
        }
    }
    /// Whether any tab is still loading, which is also what decides between a
    /// polling and a waiting event loop.
    fn is_loading(&self) -> bool {
        self.model
            .tabs
            .iter()
            .any(|tab| tab.snapshot.is_none() && tab.error.is_none())
    }
    fn layout(&self) -> Layout {
        layout(
            self.canvas.width as i32,
            self.canvas.height as i32,
            &self.model,
            self.view,
        )
    }
    fn clamp_scrolls(&mut self) {
        let layout = self.layout();
        self.view.tree_scroll = self
            .view
            .tree_scroll
            .clamp(0, (self.tree_content - layout.tree_list.h).max(0));
        self.view.panel_scroll = self
            .view
            .panel_scroll
            .clamp(0, (self.panel_content - layout.panel.h).max(0));
    }
    fn redraw(&mut self) {
        self.poll_loads();
        self.tick = self.tick.wrapping_add(1);
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        if self.canvas.width != size.width || self.canvas.height != size.height {
            self.canvas.resize(size.width, size.height);
        }
        self.canvas.set_scale(self.view.scale);
        // One measuring pass so scroll offsets can be clamped against real
        // content height, then a second pass with the corrected offsets.
        let (tree, panel) = render_app(
            &mut self.canvas,
            &self.model,
            self.view,
            &self.path_input,
            self.status.as_deref(),
            self.load_started
                .map_or(0.0, |start| start.elapsed().as_secs_f32()),
            self.tick,
        );
        self.tree_content = tree;
        self.panel_content = panel;
        let before = (self.view.tree_scroll, self.view.panel_scroll);
        self.clamp_scrolls();
        if before != (self.view.tree_scroll, self.view.panel_scroll) {
            let (tree, panel) = render_app(
                &mut self.canvas,
                &self.model,
                self.view,
                &self.path_input,
                self.status.as_deref(),
                self.load_started
                    .map_or(0.0, |start| start.elapsed().as_secs_f32()),
                self.tick,
            );
            self.tree_content = tree;
            self.panel_content = panel;
        }
        if let Some(surface) = &mut self.surface
            && let (Some(width), Some(height)) =
                (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        {
            let _ = surface.resize(width, height);
            if let Ok(mut buffer) = surface.buffer_mut() {
                buffer.copy_from_slice(&self.canvas.pixels);
                let _ = buffer.present();
            }
        }
    }
    fn visible_keys(&self) -> Vec<String> {
        tree_rows(&self.model)
            .into_iter()
            .map(|(_, key)| key)
            .collect()
    }
    fn set_scale(&mut self, scale: i32) {
        let scale = scale.clamp(MIN_SCALE, MAX_SCALE);
        if scale != self.view.scale {
            self.view.scale = scale;
            self.canvas.set_scale(scale);
        }
    }
    fn move_selection(&mut self, delta: isize) {
        let keys = self.visible_keys();
        if keys.is_empty() {
            return;
        }
        let selected = self.model.selected().map(|record| record.key.as_str());
        let current = selected
            .and_then(|key| keys.iter().position(|candidate| candidate == key))
            .unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, keys.len() as isize - 1) as usize;
        self.model.select(keys[next].clone());
        self.reveal_selection(next, keys.len());
    }
    /// Keeps the selected row inside the visible list after keyboard movement.
    fn reveal_selection(&mut self, index: usize, _total: usize) {
        let layout = self.layout();
        let row = layout.metrics.row;
        let top = index as i32 * row;
        let visible = (layout.tree_list.h - layout.metrics.pad * 2).max(row);
        if top < self.view.tree_scroll {
            self.view.tree_scroll = top;
        } else if top + row > self.view.tree_scroll + visible {
            self.view.tree_scroll = top + row - visible;
        }
    }
    fn scroll_by(&mut self, amount: i32) {
        let layout = self.layout();
        let (x, y) = self.pointer;
        if layout.tree_list.contains(x, y) || layout.tree_header.contains(x, y) {
            self.view.tree_scroll -= amount;
        } else {
            self.view.panel_scroll -= amount;
        }
        self.clamp_scrolls();
    }
    fn press(&mut self) {
        let layout = self.layout();
        let (x, y) = self.pointer;
        if layout.splitter.inset(-layout.metrics.pad).contains(x, y) {
            self.dragging_split = true;
        }
    }
    fn click(&mut self) {
        let layout = self.layout();
        let (x, y) = self.pointer;
        if layout.open_button.is_some_and(|rect| rect.contains(x, y)) {
            self.begin_open_prompt();
            return;
        }
        if let Some((_, index)) = layout.tabs.iter().find(|(rect, _)| rect.contains(x, y)) {
            self.model.active_tab = Some(*index);
            self.view.tree_scroll = 0;
            self.view.panel_scroll = 0;
            self.sync_title();
            return;
        }
        if layout.header.contains(x, y) {
            return;
        }
        if let Some((_, panel, _)) = layout.buttons.iter().find(|(rect, ..)| rect.contains(x, y)) {
            self.model.panel = *panel;
            self.view.panel_scroll = 0;
            return;
        }
        if layout.filter_box.contains(x, y) {
            self.view.filter_editing = true;
            return;
        }
        if layout.filter_bar.contains(x, y) {
            return;
        }
        if layout.mode_button.contains(x, y) {
            self.model.mode = match self.model.mode {
                TreeMode::Planning => TreeMode::Knowledge,
                TreeMode::Knowledge => TreeMode::Planning,
            };
            self.view.tree_scroll = 0;
            return;
        }
        if layout.tree_list.contains(x, y) {
            let metrics = layout.metrics;
            let local = y as i32 - (layout.tree_list.y + metrics.pad) + self.view.tree_scroll;
            if local < 0 {
                return;
            }
            let index = (local / metrics.row) as usize;
            if let Some(key) = self.visible_keys().get(index) {
                self.model.select(key.clone());
                self.view.panel_scroll = 0;
            }
        }
    }
    fn drag(&mut self) {
        if !self.dragging_split {
            return;
        }
        let width = self.canvas.width.max(1) as f64;
        self.view.split = (self.pointer.0 / width).clamp(0.12, 0.75);
    }
}

impl ApplicationHandler for NativeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("AKR Review")
            .with_inner_size(PhysicalSize::new(1280, 800))
            .with_min_inner_size(PhysicalSize::new(640, 420))
            .with_resizable(true);
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .expect("create AKR review window"),
        );
        // Start at a scale that suits the display: HiDPI screens and large
        // windows get bigger type, and the user can still zoom from there.
        let size = window.inner_size();
        self.view.scale = default_scale(window.scale_factor(), size.width, size.height);
        self.canvas.set_scale(self.view.scale);
        let context = Context::new(window.clone()).expect("create software presentation context");
        let surface =
            Surface::new(&context, window.clone()).expect("create software presentation surface");
        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
        self.sync_title();
        for tab in 0..self.model.tabs.len() {
            self.request_load(tab);
        }
        self.redraw();
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) | WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.control_held = modifiers.state().control_key();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = (position.x, position.y);
                if self.dragging_split {
                    self.drag();
                    self.redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (lines, pixels) = match delta {
                    MouseScrollDelta::LineDelta(_, y) => {
                        (y, y * self.canvas.char_height() as f32 * 3.0)
                    }
                    MouseScrollDelta::PixelDelta(point) => (point.y as f32 / 40.0, point.y as f32),
                };
                if self.control_held {
                    if lines.abs() > 0.01 {
                        self.set_scale(self.view.scale + lines.signum() as i32);
                    }
                } else {
                    self.scroll_by(pixels as i32);
                }
                self.redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.press();
                self.redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if self.dragging_split {
                    self.dragging_split = false;
                } else {
                    self.click();
                }
                self.redraw();
            }
            WindowEvent::DroppedFile(path) => {
                self.open_workspace(&path);
                self.view.path_editing = false;
                self.redraw();
            }
            WindowEvent::Ime(winit::event::Ime::Commit(text)) if self.view.path_editing => {
                self.path_input.push_str(&text);
                self.redraw();
            }
            WindowEvent::Ime(winit::event::Ime::Commit(text)) if self.view.filter_editing => {
                self.model.filter.query.push_str(&text);
                self.redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let physical = event.physical_key;
                let logical = event.logical_key;
                let editing = self.view.filter_editing;
                let page = (self.layout().panel.h - self.layout().metrics.row).max(1);
                // The open prompt is modal: it takes every key until it is
                // submitted or cancelled, so a typed path cannot half-trigger
                // the single-letter shortcuts.
                if self.view.path_editing {
                    match logical {
                        Key::Named(NamedKey::Escape) => {
                            self.view.path_editing = false;
                            self.path_input.clear();
                        }
                        Key::Named(NamedKey::Enter) => self.submit_open_prompt(),
                        Key::Named(NamedKey::Backspace) => {
                            self.path_input.pop();
                        }
                        Key::Named(NamedKey::Space) => self.path_input.push(' '),
                        Key::Character(ref value) => self.path_input.push_str(value),
                        _ => {}
                    }
                    self.redraw();
                    return;
                }
                match logical {
                    Key::Named(NamedKey::Escape) => self.view.filter_editing = false,
                    Key::Named(NamedKey::Enter) if editing => self.view.filter_editing = false,
                    Key::Named(NamedKey::Backspace) if editing => {
                        self.model.filter.query.pop();
                        self.view.tree_scroll = 0;
                    }
                    Key::Named(NamedKey::Space) if editing => self.model.filter.query.push(' '),
                    Key::Named(NamedKey::ArrowDown) if !editing => self.move_selection(1),
                    Key::Named(NamedKey::ArrowUp) if !editing => self.move_selection(-1),
                    Key::Named(NamedKey::PageDown) if !editing => self.scroll_by(-page),
                    Key::Named(NamedKey::PageUp) if !editing => self.scroll_by(page),
                    Key::Named(NamedKey::Home) if !editing => {
                        self.view.panel_scroll = 0;
                        self.view.tree_scroll = 0;
                    }
                    Key::Named(NamedKey::End) if !editing => self.scroll_by(-i32::MAX / 2),
                    // Zoom works with or without control so it is reachable on
                    // keyboards where the shifted forms differ.
                    Key::Character(ref value)
                        if matches!(value.as_str(), "+" | "=") && !editing =>
                    {
                        self.set_scale(self.view.scale + 1)
                    }
                    Key::Character(ref value)
                        if matches!(value.as_str(), "-" | "_") && !editing =>
                    {
                        self.set_scale(self.view.scale - 1)
                    }
                    Key::Character(ref value) if value == "0" && !editing => {
                        let scale = self
                            .window
                            .as_ref()
                            .map(|window| {
                                let size = window.inner_size();
                                default_scale(window.scale_factor(), size.width, size.height)
                            })
                            .unwrap_or(MIN_SCALE);
                        self.set_scale(scale);
                    }
                    Key::Character(ref value) if value == "/" && !editing => {
                        self.view.filter_editing = true
                    }
                    Key::Character(ref value) if editing => {
                        self.model.filter.query.push_str(value);
                        self.view.tree_scroll = 0;
                    }
                    Key::Character(ref value) if value.eq_ignore_ascii_case("o") => {
                        self.begin_open_prompt()
                    }
                    Key::Character(ref value) if value.eq_ignore_ascii_case("w") => {
                        self.close_active_workspace()
                    }
                    Key::Character(ref value) if value.eq_ignore_ascii_case("r") => {
                        self.request_active_load()
                    }
                    Key::Character(ref value) if value.eq_ignore_ascii_case("p") => {
                        self.model.mode = TreeMode::Planning;
                        self.view.tree_scroll = 0;
                    }
                    Key::Character(ref value) if value.eq_ignore_ascii_case("k") => {
                        self.model.mode = TreeMode::Knowledge;
                        self.view.tree_scroll = 0;
                    }
                    Key::Character(ref value) if value.eq_ignore_ascii_case("d") => {
                        self.model.panel = Panel::Dashboard;
                        self.view.panel_scroll = 0;
                    }
                    Key::Character(ref value) if value.eq_ignore_ascii_case("i") => {
                        self.model.panel = Panel::Inspector;
                        self.view.panel_scroll = 0;
                    }
                    Key::Character(ref value) if value.eq_ignore_ascii_case("l") => {
                        self.model.panel = Panel::Relations;
                        self.view.panel_scroll = 0;
                    }
                    Key::Character(ref value) if value.eq_ignore_ascii_case("g") => {
                        self.model.panel = Panel::Git;
                        self.view.panel_scroll = 0;
                    }
                    _ if !editing && matches!(physical, PhysicalKey::Code(KeyCode::Tab)) => {
                        self.model.cycle_tab();
                        self.view.tree_scroll = 0;
                        self.view.panel_scroll = 0;
                        self.sync_title();
                    }
                    _ => {}
                }
                self.redraw();
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.poll_loads();
        // Animate — and keep draining the load channel — only while something
        // is in flight. Idle, the shell waits for input instead of repainting
        // a static frame as fast as the display allows.
        if self.is_loading() {
            event_loop.set_control_flow(ControlFlow::Poll);
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

/// Paints one frame and returns the content height of the tree list and of the
/// active panel, so the caller can clamp scrolling to real content.
fn render_app(
    canvas: &mut Canvas,
    model: &AppModel,
    view: ViewState,
    path_input: &str,
    status: Option<&str>,
    elapsed: f32,
    tick: u64,
) -> (i32, i32) {
    canvas.clear(BG);
    let width = canvas.width as i32;
    let height = canvas.height as i32;
    let layout = layout(width, height, model, view);
    let metrics = layout.metrics;

    // Title bar with one clickable chip per workspace tab.
    canvas.rect(0, 0, width, layout.header.h, SURFACE);
    canvas.rect(0, layout.header.h - 1, width, 1, LINE);
    canvas.text(
        metrics.pad,
        (layout.header.h - metrics.cw) / 2,
        "AKR REVIEW",
        ACCENT,
    );
    for (rect, index) in &layout.tabs {
        let tab = &model.tabs[*index];
        button(
            canvas,
            *rect,
            metrics,
            &workspace_label(tab.workspace.as_path()),
            model.active_tab == Some(*index),
        );
    }
    if let Some(rect) = layout.open_button {
        button(canvas, rect, metrics, "+ Open", false);
    }

    // Filter row plus the panel switcher — replaced by the open prompt while
    // one is being typed, since the prompt owns the keyboard anyway.
    canvas.rect(0, layout.filter_bar.y, width, layout.filter_bar.h, SURFACE);
    canvas.rect(
        0,
        layout.filter_bar.y + layout.filter_bar.h - 1,
        width,
        1,
        LINE,
    );
    if view.path_editing {
        let box_rect = Rect {
            x: metrics.pad,
            y: layout.filter_bar.y + metrics.pad / 2,
            w: width - metrics.pad * 2,
            h: layout.filter_bar.h - metrics.pad,
        };
        canvas.rect(box_rect.x, box_rect.y, box_rect.w, box_rect.h, BG);
        canvas.border(box_rect.x, box_rect.y, box_rect.w, box_rect.h, ACCENT);
        let text = format!("open workspace: {path_input}_");
        // Show the tail of a long path: the end is the part being typed.
        let columns = canvas.columns(box_rect.w - metrics.pad * 2);
        let shown = if text.chars().count() > columns && columns > 1 {
            let skip = text.chars().count() - columns + 1;
            format!("<{}", text.chars().skip(skip).collect::<String>())
        } else {
            text
        };
        canvas.text(
            box_rect.x + metrics.pad,
            box_rect.y + (box_rect.h - metrics.cw) / 2,
            &shown,
            TEXT,
        );
    } else {
        let box_rect = layout.filter_box;
        canvas.rect(box_rect.x, box_rect.y, box_rect.w, box_rect.h, BG);
        canvas.border(
            box_rect.x,
            box_rect.y,
            box_rect.w,
            box_rect.h,
            if view.filter_editing { ACCENT } else { LINE },
        );
        let query = if model.filter.query.is_empty() && !view.filter_editing {
            "/ filter records".to_owned()
        } else {
            format!(
                "{}{}",
                model.filter.query,
                if view.filter_editing { "_" } else { "" }
            )
        };
        canvas.text_clipped(
            box_rect.x + metrics.pad,
            box_rect.y + (box_rect.h - metrics.cw) / 2,
            box_rect.w - metrics.pad * 2,
            &query,
            if model.filter.query.is_empty() && !view.filter_editing {
                MUTED
            } else {
                TEXT
            },
        );
        for (rect, panel, label) in &layout.buttons {
            button(
                canvas,
                *rect,
                metrics,
                label,
                model.panel == *panel || (*panel == Panel::Inspector && model.panel == Panel::Tree),
            );
        }
    }

    // Left pane: header strip, then the scrollable record list.
    canvas.rect(
        0,
        layout.tree_header.y,
        layout.tree_header.w,
        layout.tree_header.h + layout.tree_list.h,
        SURFACE,
    );
    let rows = tree_rows(model);
    let summary = match model.active_tab.and_then(|index| model.tabs.get(index)) {
        Some(tab) if tab.snapshot.is_none() && tab.error.is_none() => match &tab.phase {
            Some(phase) => format!("loading {}/{}", phase.step, phase.total),
            None => "loading".to_owned(),
        },
        _ => format!("{} records", rows.len()),
    };
    canvas.text_clipped(
        metrics.pad,
        layout.tree_header.y + (layout.tree_header.h - metrics.cw) / 2,
        layout.mode_button.x - metrics.pad * 2,
        &summary,
        MUTED,
    );
    button(
        canvas,
        layout.mode_button,
        metrics,
        mode_label(model.mode),
        true,
    );
    canvas.rect(
        0,
        layout.tree_header.y + layout.tree_header.h - 1,
        layout.tree_header.w,
        1,
        LINE,
    );

    let selected_key = model.selected().map(|record| record.key.clone());
    let tree_content = {
        let mut pane = Pane::new(canvas, layout.tree_list, metrics, view.tree_scroll);
        if rows.is_empty() {
            let active = model.active_tab.and_then(|index| model.tabs.get(index));
            pane.line(
                match active {
                    None => "no workspace open",
                    Some(tab) if tab.error.is_some() => "load failed",
                    Some(tab) if tab.snapshot.is_none() => "loading...",
                    Some(_) => "no records match",
                },
                MUTED,
            );
        }
        for (depth, key) in &rows {
            let record = model.snapshot().and_then(|snapshot| snapshot.record(key));
            let line = record.map_or_else(
                || key.clone(),
                |record| format!("{} {}", record.kind, record.title),
            );
            let selected = selected_key.as_deref() == Some(key.as_str());
            pane.selectable(depth * metrics.cw, &line, selected);
        }
        pane.content_height()
    };
    scrollbar(
        canvas,
        layout.tree_list,
        metrics,
        view.tree_scroll,
        tree_content,
    );

    // Splitter handle.
    canvas.rect(
        layout.splitter.x,
        layout.splitter.y,
        layout.splitter.w,
        layout.splitter.h,
        LINE,
    );
    canvas.rect(
        layout.splitter.x,
        layout.splitter.y + layout.splitter.h / 2 - metrics.row,
        layout.splitter.w,
        metrics.row * 2,
        THUMB,
    );

    let panel_content = {
        let mut pane = Pane::new(canvas, layout.panel, metrics, view.panel_scroll);
        let loading = model
            .active_tab
            .and_then(|index| model.tabs.get(index))
            .filter(|tab| tab.snapshot.is_none() && tab.error.is_none());
        match loading {
            // A load in flight owns the panel: none of the other views have
            // anything to say until it lands.
            Some(tab) => render_loading(&mut pane, tab, elapsed, tick),
            None => match model.panel {
                Panel::Dashboard => render_dashboard(&mut pane, model),
                Panel::Relations => render_relations(&mut pane, model),
                Panel::Git => render_git(&mut pane, model),
                Panel::Tree | Panel::Inspector => render_inspector(&mut pane, model),
            },
        }
        pane.content_height()
    };
    scrollbar(
        canvas,
        layout.panel,
        metrics,
        view.panel_scroll,
        panel_content,
    );

    // Footer: the last status message, or key hints, plus the zoom level.
    canvas.rect(0, layout.footer.y, width, layout.footer.h, SURFACE);
    canvas.rect(0, layout.footer.y, width, 1, LINE);
    let hints = if view.path_editing {
        "type a project directory   Enter open   Esc cancel   or drop a folder on the window"
    } else {
        "O open   W close   Tab switch   / filter   P/K tree   D I L G panels   R reload   +/- zoom"
    };
    let loading_line = model
        .active_tab
        .and_then(|index| model.tabs.get(index))
        .filter(|tab| tab.snapshot.is_none() && tab.error.is_none())
        .map(|tab| match &tab.phase {
            Some(phase) => format!(
                "loading {}/{}  {}  {elapsed:.1}s",
                phase.step, phase.total, phase.label
            ),
            None => format!("loading  {elapsed:.1}s"),
        });
    let footer_text = match (&loading_line, status) {
        (Some(line), _) if !view.path_editing => line.as_str(),
        (None, Some(status)) if !view.path_editing => status,
        _ => hints,
    };
    canvas.text_clipped(
        metrics.pad,
        layout.footer.y + (layout.footer.h - metrics.cw) / 2,
        width - metrics.pad * 2 - metrics.width_of("zoom x0") - metrics.pad,
        footer_text,
        if loading_line.is_some() || (status.is_some() && !view.path_editing) {
            TEXT
        } else {
            MUTED
        },
    );
    let zoom = format!("zoom x{}", view.scale);
    canvas.text(
        width - metrics.width_of(&zoom) - metrics.pad,
        layout.footer.y + (layout.footer.h - metrics.cw) / 2,
        &zoom,
        MUTED,
    );
    (tree_content, panel_content)
}

/// Draws the load progress for a tab that has no snapshot yet.
///
/// The bar is determinate over the phase list, and the elapsed seconds keep
/// climbing inside a phase, so a long git wait still looks alive rather than
/// hung. `frame` animates the marker.
fn render_loading(
    pane: &mut Pane<'_>,
    tab: &akr_gui::model::WorkspaceTab,
    elapsed: f32,
    frame: u64,
) {
    const MARKERS: [&str; 4] = ["|", "/", "-", "\\"];
    let marker = MARKERS[(frame / 8) as usize % MARKERS.len()];
    pane.line(&format!("{marker} Loading workspace"), ACCENT);
    pane.gap(1);
    pane.paragraph(&tab.workspace.display().to_string(), MUTED);
    pane.gap(1);
    let (step, total, label) = match &tab.phase {
        Some(phase) => (phase.step, phase.total, phase.label.as_str()),
        None => (0, 5, "starting"),
    };
    pane.line(&format!("step {step} of {total}: {label}"), TEXT);
    pane.progress(step, total);
    pane.line(&format!("{elapsed:.1}s elapsed"), MUTED);
    pane.gap(1);
    pane.paragraph(
        "Most of this is git: freshness asks the repository which commits touched \
         each watched path. Large histories take longer, and the answer is cached \
         for the rest of the load.",
        MUTED,
    );
}

/// Renders a load error, if any, and reports whether the panel should stop.
fn render_load_error(pane: &mut Pane<'_>, model: &AppModel) -> bool {
    let Some(tab) = model.active_tab.and_then(|index| model.tabs.get(index)) else {
        pane.line("No workspace open.", TEXT);
        pane.gap(1);
        pane.paragraph(
            "Press O and type a project directory, click + Open in the title bar, \
             or drag a project folder onto this window. A path inside a project \
             resolves to the directory holding its .akr folder.",
            MUTED,
        );
        return true;
    };
    let Some(error) = &tab.error else {
        return false;
    };
    pane.line("LOAD FAILED", WARNING);
    pane.gap(1);
    pane.paragraph(error, TEXT);
    true
}

fn render_dashboard(pane: &mut Pane<'_>, model: &AppModel) {
    if render_load_error(pane, model) {
        return;
    }
    let Some(snapshot) = model.snapshot() else {
        pane.line("Loading review snapshot...", MUTED);
        return;
    };
    pane.line(&snapshot.project, ACCENT);
    pane.gap(1);
    pane.line(
        &format!(
            "{} records / {} revisions",
            snapshot.counts.records, snapshot.counts.revisions
        ),
        TEXT,
    );
    pane.line(
        &format!(
            "{} live planning / {} open questions",
            snapshot.counts.live_planning, snapshot.counts.open_questions
        ),
        TEXT,
    );
    pane.line(
        &format!(
            "{} stale / {} at risk / {} diagnostics",
            snapshot.counts.stale, snapshot.counts.at_risk, snapshot.counts.diagnostics
        ),
        if snapshot.counts.stale + snapshot.counts.at_risk + snapshot.counts.diagnostics > 0 {
            WARNING
        } else {
            MUTED
        },
    );
    pane.gap(1);
    pane.line(&format!("source graph  {}", snapshot.source_graph), MUTED);
    if let Some(head) = &snapshot.head {
        pane.line(&format!("git HEAD      {head}"), MUTED);
    }
    pane.heading("DIAGNOSTICS");
    if snapshot.diagnostics.is_empty() {
        pane.line("none", MUTED);
    }
    for diagnostic in &snapshot.diagnostics {
        pane.paragraph(
            &format!(
                "{} {}  {}",
                diagnostic.severity, diagnostic.code, diagnostic.message
            ),
            if diagnostic.severity == "error" {
                WARNING
            } else {
                MUTED
            },
        );
    }
}

fn render_inspector(pane: &mut Pane<'_>, model: &AppModel) {
    if render_load_error(pane, model) {
        return;
    }
    let Some(record) = model.selected() else {
        pane.line("Select a record to inspect", MUTED);
        return;
    };
    pane.line(&record.key, ACCENT);
    pane.line(
        &format!(
            "{} / {} / r{} / freshness: {}",
            record.kind, record.state, record.revision, record.freshness
        ),
        TEXT,
    );
    pane.gap(1);
    pane.paragraph(&record.title, TEXT);
    pane.gap(1);
    pane.paragraph(&record.body, MUTED);
    pane.heading("ACCEPTANCE");
    if record.acceptance.is_empty() {
        pane.line("none", MUTED);
    }
    for check in &record.acceptance {
        pane.paragraph(
            &format!("{}  {}  {}", check.id, check.verdict, check.statement),
            if check.verdict.starts_with("satisfied") {
                MUTED
            } else {
                WARNING
            },
        );
    }
    pane.heading("REVISION HISTORY");
    if record.history.is_empty() {
        pane.line("none", MUTED);
    }
    for revision in &record.history {
        pane.line(revision, MUTED);
    }
}

fn render_relations(pane: &mut Pane<'_>, model: &AppModel) {
    if render_load_error(pane, model) {
        return;
    }
    let Some(record) = model.selected() else {
        pane.line("Select a record to inspect relations", MUTED);
        return;
    };
    pane.line(&record.key, ACCENT);
    pane.heading("OUTBOUND RELATIONS");
    if record.relations.is_empty() {
        pane.line("none", MUTED);
    }
    for relation in &record.relations {
        pane.line(&format!("{} -> {}", relation.kind, relation.target), TEXT);
    }
    pane.heading("LOCAL NEIGHBORHOOD (2 HOPS / 12 NODES)");
    let neighbors = model
        .snapshot()
        .map(|snapshot| snapshot.neighborhood(&record.key, 2, 12))
        .unwrap_or_default();
    if neighbors.is_empty() {
        pane.line("none", MUTED);
    }
    for edge in &neighbors {
        pane.line(
            &format!("{} -{}-> {}", edge.from, edge.kind, edge.to),
            MUTED,
        );
    }
}

fn render_git(pane: &mut Pane<'_>, model: &AppModel) {
    if render_load_error(pane, model) {
        return;
    }
    let Some(record) = model.selected() else {
        pane.line("Select a record to inspect provenance", MUTED);
        return;
    };
    pane.line(&record.key, ACCENT);
    pane.heading("GIT");
    let values = [
        (
            "defined at",
            record.git.defined_at.as_deref().unwrap_or("uncommitted"),
        ),
        (
            "observed at",
            record.git.observed_at.as_deref().unwrap_or("not empirical"),
        ),
        (
            "stale cause",
            record.git.stale_cause.as_deref().unwrap_or("none"),
        ),
    ];
    for (label, value) in values {
        pane.line(&format!("{label:12} {value}"), TEXT);
    }
    pane.heading("PROVENANCE");
    if record.provenance.is_empty() {
        pane.line("none", MUTED);
    }
    for value in &record.provenance {
        pane.paragraph(value, MUTED);
    }
}

/// Picks an initial glyph scale from the display DPI and window size, so the
/// shell is readable before the user touches the zoom keys.
fn default_scale(scale_factor: f64, width: u32, height: u32) -> i32 {
    let dpi = scale_factor.round().max(1.0) as i32;
    let roomy = width >= 1800 && height >= 1100;
    (MIN_SCALE.max(dpi) + i32::from(roomy)).clamp(MIN_SCALE, MAX_SCALE)
}

fn tree_rows(model: &AppModel) -> Vec<(i32, String)> {
    let Some(snapshot) = model.snapshot() else {
        return Vec::new();
    };
    let allowed = model
        .filtered_keys()
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    match model.mode {
        TreeMode::Planning => {
            fn visit(
                node: &akr_gui::model::TreeNode,
                depth: i32,
                allowed: &std::collections::BTreeSet<String>,
                out: &mut Vec<(i32, String)>,
            ) {
                if allowed.contains(&node.key) {
                    out.push((depth, node.key.clone()));
                }
                for child in &node.children {
                    visit(child, depth + 1, allowed, out);
                }
            }
            let mut out = Vec::new();
            for root in snapshot.planning_roots() {
                visit(&root, 0, &allowed, &mut out);
            }
            out
        }
        TreeMode::Knowledge => snapshot
            .knowledge_groups()
            .into_values()
            .flatten()
            .filter(|key| allowed.contains(key))
            .map(|key| (0, key))
            .collect(),
    }
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.len() + word.len() + 1 > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspaces = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let event_loop = EventLoop::new()?;
    let mut app = NativeApp::new(workspaces);
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapping_is_deterministic() {
        assert_eq!(wrap("one two three", 7), ["one two", "three"]);
    }
    fn demo_model() -> AppModel {
        let mut model = AppModel::default();
        let tab = model.add_tab("/demo".into());
        model.set_snapshot(tab, akr_gui::model::demo_snapshot("/demo"));
        model
    }
    #[test]
    fn panes_tile_the_window_without_overlap_at_every_scale() {
        let model = demo_model();
        for scale in MIN_SCALE..=MAX_SCALE {
            for (width, height) in [(640, 420), (1280, 800), (2560, 1440)] {
                let view = ViewState {
                    scale,
                    ..ViewState::default()
                };
                let layout = layout(width, height, &model, view);
                assert_eq!(layout.header.h + layout.filter_bar.h, layout.tree_header.y);
                assert_eq!(
                    layout.tree_header.y + layout.tree_header.h,
                    layout.tree_list.y
                );
                assert_eq!(layout.tree_list.y + layout.tree_list.h, layout.footer.y);
                assert!(layout.panel.x >= layout.splitter.x + layout.splitter.w);
                assert_eq!(layout.panel.x + layout.panel.w, width);
                assert!(layout.panel.w > 0 && layout.tree_list.w > 0);
                assert!(layout.mode_button.x >= 0);
                for (rect, ..) in &layout.buttons {
                    assert!(rect.x >= layout.filter_box.x + layout.filter_box.w);
                    assert!(rect.x + rect.w <= width);
                }
            }
        }
    }
    #[test]
    fn the_split_is_clamped_so_both_panes_stay_usable() {
        let model = demo_model();
        let narrow = layout(
            900,
            600,
            &model,
            ViewState {
                split: 0.99,
                ..ViewState::default()
            },
        );
        assert!(narrow.panel.w >= narrow.metrics.cw * 20);
        let wide = layout(
            900,
            600,
            &model,
            ViewState {
                split: 0.01,
                ..ViewState::default()
            },
        );
        assert!(wide.tree_list.w >= wide.metrics.cw * 12);
    }
    #[test]
    fn a_frame_reports_content_height_and_stays_inside_the_surface() {
        let model = demo_model();
        let mut canvas = Canvas::new(900, 600);
        canvas.set_scale(3);
        let view = ViewState {
            scale: 3,
            ..ViewState::default()
        };
        let (tree, panel) = render_app(&mut canvas, &model, view, "", None, 0.0, 0);
        assert!(tree > 0 && panel > 0);
        assert_eq!(canvas.pixels.len(), 900 * 600);
        // Scrolling far past the end must not panic or paint outside the pane.
        let scrolled = ViewState {
            tree_scroll: 100_000,
            panel_scroll: 100_000,
            ..view
        };
        let (tree_again, _) = render_app(&mut canvas, &model, scrolled, "", Some("opened"), 0.0, 0);
        assert_eq!(tree, tree_again);
        // The open prompt replaces the filter row without disturbing the panes.
        let prompting = ViewState {
            path_editing: true,
            ..view
        };
        let (tree_prompt, _) = render_app(
            &mut canvas,
            &model,
            prompting,
            r"D:\some\very\long\path\that\overflows\the\prompt\box",
            None,
            0.0,
            0,
        );
        assert_eq!(tree, tree_prompt);
    }
    #[test]
    fn a_tab_without_a_snapshot_shows_progress_rather_than_an_empty_panel() {
        let mut model = AppModel::default();
        model.add_tab("/loading".into());
        let mut canvas = Canvas::new(900, 600);
        canvas.set_scale(MIN_SCALE);
        let (_, before) = render_app(&mut canvas, &model, ViewState::default(), "", None, 0.4, 0);
        assert!(before > 0, "an in-flight load must render something");
        // A reported phase adds the step line without changing the panel shape.
        model.tabs[0].phase = Some(akr_gui::model::LoadPhase {
            label: "checking git freshness".into(),
            step: 3,
            total: 5,
        });
        let (_, during) = render_app(&mut canvas, &model, ViewState::default(), "", None, 9.6, 24);
        assert_eq!(before, during);
        // Once the snapshot lands the panel is the normal view again.
        model.set_snapshot(0, akr_gui::model::demo_snapshot("/loading"));
        let (_, after) = render_app(&mut canvas, &model, ViewState::default(), "", None, 0.0, 0);
        assert_ne!(before, after);
    }
    #[test]
    fn an_empty_shell_renders_its_own_instructions() {
        let model = AppModel::default();
        let mut canvas = Canvas::new(900, 600);
        canvas.set_scale(MIN_SCALE);
        let (_, panel) = render_app(&mut canvas, &model, ViewState::default(), "", None, 0.0, 0);
        assert!(panel > 0, "the no-workspace state must still say something");
    }
    #[test]
    fn a_path_resolves_to_the_directory_holding_the_ledger() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        assert!(repo.join(".akr").is_dir(), "fixture assumption");
        // A file inside the project resolves to the project itself.
        let from_file = workspace_root(&repo.join("crates/akr-gui/src/main.rs")).unwrap();
        assert_eq!(from_file, workspace_root(repo).unwrap());
        // A nested directory does too.
        let from_dir = workspace_root(&repo.join("crates/akr-gui")).unwrap();
        assert_eq!(from_dir, from_file);
        assert!(!from_file.to_string_lossy().starts_with(r"\\?\"));
        assert!(workspace_root(Path::new("no/such/place/anywhere")).is_err());
    }
    #[test]
    fn default_scale_grows_with_dpi_and_window_size() {
        assert_eq!(default_scale(1.0, 1280, 800), MIN_SCALE);
        assert_eq!(default_scale(2.0, 1280, 800), 2);
        assert_eq!(default_scale(2.0, 1920, 1200), 3);
        assert_eq!(default_scale(9.0, 3840, 2160), MAX_SCALE);
    }
    #[test]
    fn planning_rows_follow_the_relation_hierarchy() {
        let mut model = AppModel::default();
        let tab = model.add_tab("/demo".into());
        model.set_snapshot(tab, akr_gui::model::demo_snapshot("/demo"));
        assert_eq!(
            tree_rows(&model)
                .iter()
                .map(|(_, key)| key.as_str())
                .collect::<Vec<_>>(),
            [
                "akr.milestone.human-record-review",
                "akr.work.desktop-review-gui",
                "akr.work.review-snapshot"
            ]
        );
    }
}
