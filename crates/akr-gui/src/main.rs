#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#![allow(missing_docs)]

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use akr_gui::model::{
    AcceptanceCheck, Diagnostic, GitMetadata, Record, Relation, ReviewCounts, ReviewSnapshot,
    SnapshotError, WorkspaceLoader,
};
use akr_gui::render::Canvas;
use akr_gui::ui::{AppModel, Panel, TreeMode};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

const BG: u32 = 0x0010_1720;
const SURFACE: u32 = 0x001a_2633;
const LINE: u32 = 0x0032_4658;
const TEXT: u32 = 0x00d8_e3ed;
const MUTED: u32 = 0x008d_a4b8;
const ACCENT: u32 = 0x0056_b6d9;
const SELECTED: u32 = 0x0033_5870;
const WARNING: u32 = 0x00ed_b55f;
const ROW_HEIGHT: i32 = 16;

#[derive(Clone)]
struct AkrLoader;
impl WorkspaceLoader for AkrLoader {
    fn load(&self, workspace: &Path) -> Result<ReviewSnapshot, SnapshotError> {
        let source = akr_cli::review_snapshot::ReviewSnapshot::load(
            workspace,
            akr_cli::review_snapshot::ReviewOptions::default(),
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

struct LoadResult {
    tab: usize,
    generation: u64,
    result: Result<ReviewSnapshot, SnapshotError>,
}

struct NativeApp {
    window: Option<Arc<Window>>,
    context: Option<Context<Arc<Window>>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    canvas: Canvas,
    model: AppModel,
    scroll: i32,
    pointer: (f64, f64),
    loads: mpsc::Receiver<LoadResult>,
    sender: mpsc::Sender<LoadResult>,
    loader: Arc<dyn WorkspaceLoader>,
    filter_editing: bool,
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
            scroll: 0,
            pointer: (0.0, 0.0),
            loads,
            sender,
            loader: Arc::new(AkrLoader),
            filter_editing: false,
        }
    }
    fn request_load(&mut self, tab: usize) {
        let Some(workspace) = self.model.tabs.get_mut(tab).map(|tab| {
            tab.load_generation += 1;
            tab.workspace.clone()
        }) else {
            return;
        };
        let generation = self.model.tabs[tab].load_generation;
        let sender = self.sender.clone();
        let loader = self.loader.clone();
        std::thread::spawn(move || {
            let result = loader.load(&workspace);
            let _ = sender.send(LoadResult {
                tab,
                generation,
                result,
            });
        });
    }
    fn request_active_load(&mut self) {
        if let Some(tab) = self.model.active_tab {
            self.request_load(tab);
        }
    }
    fn poll_loads(&mut self) {
        while let Ok(message) = self.loads.try_recv() {
            let Some(tab) = self.model.tabs.get(message.tab) else {
                continue;
            };
            if tab.load_generation != message.generation {
                continue;
            }
            match message.result {
                Ok(snapshot) => self.model.set_snapshot(message.tab, snapshot),
                Err(error) => self.model.fail_load(message.tab, error.message),
            }
        }
    }
    fn redraw(&mut self) {
        self.poll_loads();
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
        render_app(
            &mut self.canvas,
            &self.model,
            self.scroll,
            self.filter_editing,
        );
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
        self.model.filtered_keys()
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
    }
    fn click(&mut self) {
        let (x, y) = self.pointer;
        if y < 24.0 {
            self.model.cycle_tab();
            return;
        }
        if y < 48.0 {
            self.filter_editing = true;
            return;
        }
        if x < 330.0 && y >= 72.0 {
            let index = ((y as i32 - 72 + self.scroll) / ROW_HEIGHT).max(0) as usize;
            if let Some(key) = self.visible_keys().get(index) {
                self.model.select(key.clone());
            }
        }
    }
}

impl ApplicationHandler for NativeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("AKR Review")
            .with_inner_size(PhysicalSize::new(1280, 800));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .expect("create AKR review window"),
        );
        let context = Context::new(window.clone()).expect("create software presentation context");
        let surface =
            Surface::new(&context, window.clone()).expect("create software presentation surface");
        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
        for tab in 0..self.model.tabs.len() {
            self.request_load(tab);
        }
        self.redraw();
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) | WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::CursorMoved { position, .. } => self.pointer = (position.x, position.y),
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => (y * 40.0) as i32,
                    MouseScrollDelta::PixelDelta(point) => point.y as i32,
                };
                self.scroll = (self.scroll - amount).max(0);
                self.redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.click();
                self.redraw();
            }
            WindowEvent::Ime(winit::event::Ime::Commit(text)) if self.filter_editing => {
                self.model.filter.query.push_str(&text);
                self.redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let physical = event.physical_key;
                let logical = event.logical_key;
                match logical {
                    Key::Named(NamedKey::Escape) => self.filter_editing = false,
                    Key::Named(NamedKey::Backspace) if self.filter_editing => {
                        self.model.filter.query.pop();
                    }
                    Key::Named(NamedKey::ArrowDown) if !self.filter_editing => {
                        self.move_selection(1)
                    }
                    Key::Named(NamedKey::ArrowUp) if !self.filter_editing => {
                        self.move_selection(-1)
                    }
                    Key::Character(ref value) if value == "/" => self.filter_editing = true,
                    Key::Character(ref value)
                        if !self.filter_editing && value.eq_ignore_ascii_case("r") =>
                    {
                        self.request_active_load()
                    }
                    Key::Character(ref value)
                        if !self.filter_editing && value.eq_ignore_ascii_case("p") =>
                    {
                        self.model.mode = TreeMode::Planning
                    }
                    Key::Character(ref value)
                        if !self.filter_editing && value.eq_ignore_ascii_case("k") =>
                    {
                        self.model.mode = TreeMode::Knowledge
                    }
                    Key::Character(ref value)
                        if !self.filter_editing && value.eq_ignore_ascii_case("d") =>
                    {
                        self.model.panel = Panel::Dashboard
                    }
                    Key::Character(ref value)
                        if !self.filter_editing && value.eq_ignore_ascii_case("i") =>
                    {
                        self.model.panel = Panel::Inspector
                    }
                    Key::Character(ref value)
                        if !self.filter_editing && value.eq_ignore_ascii_case("l") =>
                    {
                        self.model.panel = Panel::Relations
                    }
                    Key::Character(ref value)
                        if !self.filter_editing && value.eq_ignore_ascii_case("g") =>
                    {
                        self.model.panel = Panel::Git
                    }
                    _ if !self.filter_editing
                        && matches!(physical, PhysicalKey::Code(KeyCode::Tab)) =>
                    {
                        self.model.cycle_tab()
                    }
                    _ => {}
                }
                self.redraw();
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        self.poll_loads();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn render_app(canvas: &mut Canvas, model: &AppModel, scroll: i32, filter_editing: bool) {
    canvas.clear(BG);
    let width = canvas.width as i32;
    let height = canvas.height as i32;
    canvas.rect(0, 0, width, 24, SURFACE);
    canvas.border(0, 0, width, 24, LINE);
    let tabs = model
        .tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            format!(
                "{}{}",
                if model.active_tab == Some(index) {
                    "["
                } else {
                    " "
                },
                tab.workspace
                    .file_name()
                    .and_then(|part| part.to_str())
                    .unwrap_or("workspace")
            )
        })
        .collect::<Vec<_>>()
        .join("  ");
    canvas.text_clipped(8, 8, width - 16, &format!("AKR REVIEW  {tabs}"), TEXT);
    canvas.rect(0, 24, width, 24, SURFACE);
    canvas.text_clipped(
        8,
        32,
        width - 16,
        &format!(
            "/ filter: {}{}   P/K tree  D dashboard  I detail  L relations  G git  R reload",
            model.filter.query,
            if filter_editing { "_" } else { "" }
        ),
        MUTED,
    );
    canvas.border(0, 47, width, 1, LINE);
    let left_width = (width / 3).clamp(260, 420);
    let right_x = left_width + 1;
    canvas.rect(0, 48, left_width, height - 48, SURFACE);
    canvas.border(left_width, 48, 1, height - 48, LINE);
    let heading = match model.mode {
        TreeMode::Planning => "PLANNING TREE",
        TreeMode::Knowledge => "KNOWLEDGE RECORDS",
    };
    canvas.text(8, 56, heading, TEXT);
    let rows = tree_rows(model);
    for (index, (depth, key)) in rows.iter().enumerate() {
        let y = 72 + index as i32 * ROW_HEIGHT - scroll;
        if y < 64 || y > height - 8 {
            continue;
        }
        let selected = model.selected().is_some_and(|record| record.key == *key);
        if selected {
            canvas.rect(4, y - 2, left_width - 8, ROW_HEIGHT, SELECTED);
        }
        let line = model
            .snapshot()
            .and_then(|snapshot| snapshot.record(key))
            .map(|record| format!("{} {}", record.kind, record.title))
            .unwrap_or_else(|| key.clone());
        canvas.text_clipped(
            8 + depth * 12,
            y,
            left_width - 16 - depth * 12,
            &line,
            if selected { TEXT } else { MUTED },
        );
    }
    let panel = (right_x + 12, 56, width - right_x - 24, height - 64);
    match model.panel {
        Panel::Dashboard => render_dashboard(canvas, model, panel.0, panel.1, panel.2),
        Panel::Relations => render_relations(canvas, model, panel.0, panel.1, panel.2),
        Panel::Git => render_git(canvas, model, panel.0, panel.1, panel.2),
        Panel::Tree | Panel::Inspector => {
            render_inspector(canvas, model, panel.0, panel.1, panel.2, panel.3)
        }
    }
}

fn render_dashboard(canvas: &mut Canvas, model: &AppModel, x: i32, y: i32, width: i32) {
    let Some(tab) = model.active_tab.and_then(|index| model.tabs.get(index)) else {
        return;
    };
    if let Some(error) = &tab.error {
        canvas.text_clipped(x, y, width, "LOAD FAILED", WARNING);
        canvas.text_clipped(x, y + ROW_HEIGHT, width, error, TEXT);
        return;
    }
    let Some(snapshot) = model.snapshot() else {
        canvas.text(x, y, "Loading review snapshot...", MUTED);
        return;
    };
    canvas.text_clipped(x, y, width, &snapshot.project, ACCENT);
    canvas.text_clipped(
        x,
        y + ROW_HEIGHT * 2,
        width,
        &format!(
            "{} records / {} revisions / {} live planning / {} open questions",
            snapshot.counts.records,
            snapshot.counts.revisions,
            snapshot.counts.live_planning,
            snapshot.counts.open_questions
        ),
        TEXT,
    );
    canvas.text_clipped(
        x,
        y + ROW_HEIGHT * 3,
        width,
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
    canvas.text_clipped(
        x,
        y + ROW_HEIGHT * 5,
        width,
        &format!("source graph  {}", snapshot.source_graph),
        MUTED,
    );
    if let Some(head) = &snapshot.head {
        canvas.text_clipped(
            x,
            y + ROW_HEIGHT * 6,
            width,
            &format!("git HEAD      {head}"),
            MUTED,
        );
    }
    canvas.text(x, y + ROW_HEIGHT * 8, "DIAGNOSTICS", ACCENT);
    for (index, diagnostic) in snapshot.diagnostics.iter().take(12).enumerate() {
        canvas.text_clipped(
            x,
            y + ROW_HEIGHT * (index as i32 + 9),
            width,
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

fn render_inspector(
    canvas: &mut Canvas,
    model: &AppModel,
    x: i32,
    y: i32,
    width: i32,
    _height: i32,
) {
    let Some(tab) = model.active_tab.and_then(|index| model.tabs.get(index)) else {
        return;
    };
    if let Some(error) = &tab.error {
        canvas.text_clipped(x, y, width, "LOAD FAILED", WARNING);
        canvas.text_clipped(x, y + ROW_HEIGHT, width, error, TEXT);
        return;
    }
    let Some(record) = model.selected() else {
        canvas.text(x, y, "Loading review snapshot...", MUTED);
        return;
    };
    canvas.text_clipped(x, y, width, &record.key, ACCENT);
    canvas.text_clipped(
        x,
        y + ROW_HEIGHT,
        width,
        &format!(
            "{} / {} / r{} / freshness: {}",
            record.kind, record.state, record.revision, record.freshness
        ),
        TEXT,
    );
    let mut row = y + ROW_HEIGHT * 3;
    canvas.text_clipped(x, row, width, &record.title, TEXT);
    row += ROW_HEIGHT * 2;
    for line in wrap(&record.body, (width / 8) as usize).into_iter().take(6) {
        canvas.text_clipped(x, row, width, &line, MUTED);
        row += ROW_HEIGHT;
    }
    row += ROW_HEIGHT;
    canvas.text(x, row, "ACCEPTANCE", ACCENT);
    row += ROW_HEIGHT;
    for check in record.acceptance.iter().take(6) {
        canvas.text_clipped(
            x,
            row,
            width,
            &format!("{}  {}  {}", check.id, check.verdict, check.statement),
            if check.verdict.starts_with("satisfied") {
                MUTED
            } else {
                WARNING
            },
        );
        row += ROW_HEIGHT;
    }
    row += ROW_HEIGHT;
    canvas.text(x, row, "REVISION HISTORY", ACCENT);
    row += ROW_HEIGHT;
    for revision in record.history.iter().take(6) {
        canvas.text_clipped(x, row, width, revision, MUTED);
        row += ROW_HEIGHT;
    }
}

fn render_relations(canvas: &mut Canvas, model: &AppModel, x: i32, y: i32, width: i32) {
    let Some(record) = model.selected() else {
        canvas.text(x, y, "Select a record to inspect relations", MUTED);
        return;
    };
    canvas.text_clipped(x, y, width, &record.key, ACCENT);
    canvas.text(x, y + ROW_HEIGHT * 2, "OUTBOUND RELATIONS", ACCENT);
    for (index, relation) in record.relations.iter().take(12).enumerate() {
        canvas.text_clipped(
            x,
            y + ROW_HEIGHT * (index as i32 + 3),
            width,
            &format!("{} -> {}", relation.kind, relation.target),
            TEXT,
        );
    }
    let row = y + ROW_HEIGHT * (record.relations.len().min(12) as i32 + 5);
    canvas.text(x, row, "LOCAL NEIGHBORHOOD (2 HOPS / 12 NODES)", ACCENT);
    let neighbors = model
        .snapshot()
        .map(|snapshot| snapshot.neighborhood(&record.key, 2, 12))
        .unwrap_or_default();
    for (index, edge) in neighbors.iter().take(12).enumerate() {
        canvas.text_clipped(
            x,
            row + ROW_HEIGHT * (index as i32 + 1),
            width,
            &format!("{} -{}-> {}", edge.from, edge.kind, edge.to),
            MUTED,
        );
    }
}

fn render_git(canvas: &mut Canvas, model: &AppModel, x: i32, y: i32, width: i32) {
    let Some(record) = model.selected() else {
        canvas.text(x, y, "Select a record to inspect provenance", MUTED);
        return;
    };
    canvas.text_clipped(x, y, width, &record.key, ACCENT);
    canvas.text(x, y + ROW_HEIGHT * 2, "GIT", ACCENT);
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
    for (index, (label, value)) in values.iter().enumerate() {
        canvas.text_clipped(
            x,
            y + ROW_HEIGHT * (index as i32 + 3),
            width,
            &format!("{label:12} {value}"),
            TEXT,
        );
    }
    canvas.text(x, y + ROW_HEIGHT * 7, "PROVENANCE", ACCENT);
    for (index, value) in record.provenance.iter().take(12).enumerate() {
        canvas.text_clipped(x, y + ROW_HEIGHT * (index as i32 + 8), width, value, MUTED);
    }
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
