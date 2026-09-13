//! The agent panel: ask Codex for a musical change, watch it work, and revert any
//! command an agent ran against this window, whether it came from Codex, an MCP
//! client or the CLI. Layout follows the agent panel in
//! `design/Ondera Arrangement.dc.html`: a header, the current action, the log of
//! changes this session and one prompt. Settings live in the Agent menu.

use crate::{app::Ondera, theme::*};
use eframe::egui::{self, pos2, vec2, Align2, Color32, Id, Rect, Sense, Vec2};
use ondera_engine::{control, model::Session, store::Command, Result};
use serde_json::{json, Value};
use std::{collections::VecDeque, path::Path, time::Instant};

const HISTORY_LIMIT: usize = 64;
const DETAIL_LIMIT: usize = 24_000;
const PAD: f32 = 14.0;
const ENTRY_H: f32 = 44.0;
const CHIP: f32 = 26.0;

#[derive(Default)]
pub(crate) struct AgentPanel {
    pub open: bool,
    runner: crate::agent_runner::Runner,
    history: VecDeque<Activity>,
    sequence: u64,
    last_request: Option<Instant>,
    task: Option<Task>,
}

/// The running or most recent Codex task.
struct Task {
    started: Instant,
    edits: usize,
}

struct Activity {
    sequence: u64,
    title: String,
    /// The command as the CLI would spell it.
    detail: String,
    output: String,
    color: Color32,
    succeeded: bool,
    running: bool,
    expanded: bool,
    before: u64,
    after: u64,
    depth_before: usize,
    depth_after: usize,
}

impl Activity {
    fn mutated(&self) -> bool {
        self.depth_after > self.depth_before
    }
    fn applied(&self, undo_depth: usize) -> bool {
        undo_depth >= self.depth_after
    }
}

struct Connection {
    port: Option<u16>,
    discovery: std::path::PathBuf,
}

enum Action {
    Send,
    StopTask,
    Revert(u64),
    Redo(u64),
}

/// Run widgets inside `rect` without advancing the parent's cursor, so a button
/// laid over a painted row leaves the row's own allocation intact.
fn place<R>(ui: &mut egui::Ui, rect: Rect, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    add(&mut child)
}

fn small_button(ui: &mut egui::Ui, label: &str, face_kind: Face) -> egui::Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), font(FS_VALUE, Weight::SemiBold), INK);
    let size = vec2(galley.size().x + 18.0, 22.0);
    let response = button(ui, size, face_kind, R_MD, |p, r, ink| {
        p.galley(r.center() - galley.size() / 2.0, galley.clone(), ink);
    });
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response
}

impl AgentPanel {
    fn working(&self) -> bool {
        self.runner.running()
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        connection: &Connection,
        undo_depth: usize,
    ) -> Option<Action> {
        let mut action = None;
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let width = ui.available_width();
        let working = self.working();

        // Header.
        let (bar, _) = ui.allocate_exact_size(vec2(width, AGENT_HEADER), Sense::hover());
        {
            let p = ui.painter();
            hline(p, bar.left(), bar.right(), bar.bottom() - 1.0, black(0.5));
            accent_dot(p, pos2(bar.left() + 20.0, bar.center().y), 4.0, working);
            let title = p.layout_no_wrap("Agent".into(), font(FS_PROSE, Weight::Bold), INK);
            let title_x = bar.left() + 34.0;
            p.galley(
                pos2(title_x, bar.center().y - title.size().y / 2.0),
                title.clone(),
                INK,
            );
            let status = if working {
                "working · via MCP".to_string()
            } else if connection.port.is_none() {
                "bridge off".to_string()
            } else {
                "idle · via MCP".to_string()
            };
            p.text(
                pos2(title_x + title.size().x + 10.0, bar.center().y),
                Align2::LEFT_CENTER,
                status,
                mono_font(FS_SMALL),
                FAINT,
            );
        }
        place(
            ui,
            Rect::from_min_max(
                pos2(
                    bar.right() - 16.0 - 24.0,
                    bar.top() + (AGENT_HEADER - 22.0) / 2.0,
                ),
                pos2(bar.right() - 16.0, bar.bottom()),
            ),
            |ui| {
                if button(ui, vec2(24.0, 22.0), Face::Raised, R_MD, |p, r, ink| {
                    icon(p, r, Icon::ChevronRight, ink)
                })
                .on_hover_text("Hide the agent panel")
                .clicked()
                {
                    self.open = false;
                }
            },
        );

        // Current action, or the reply from the last task.
        if working {
            self.now_card(ui, width, &mut action);
        } else if !self.runner.response.is_empty() || self.runner.error.is_some() {
            self.reply_card(ui, width);
        }

        // Log.
        let input_h = self.input_height();
        let log_h = (ui.available_height() - input_h - 12.0 - 14.0 - 12.0).max(60.0);
        ui.add_space(PAD);
        ui.horizontal(|ui| {
            ui.add_space(PAD + 2.0);
            ui.label(caps("Changes this session"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(PAD + 2.0);
                let n = self.history.len();
                ui.label(mono(
                    match n {
                        0 => "no entries".to_string(),
                        1 => "1 entry".to_string(),
                        n => format!("{n} entries"),
                    },
                    FS_SMALL,
                    FAINT,
                ));
            });
        });
        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .id_salt("agent-log")
            .max_height(log_h - 8.0 - 12.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(width);
                if self.history.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(PAD + 2.0);
                        ui.add(
                            egui::Label::new(text(
                                "Ask for a musical change below, or point an MCP client or ondera-cli at this window. Every command lands here and can be reverted.",
                                FS_SECONDARY,
                                Weight::Medium,
                                DIM,
                            ))
                            .wrap(),
                        );
                        ui.add_space(PAD + 2.0);
                    });
                }
                for entry in &mut self.history {
                    entry_row(ui, width, entry, undo_depth, &mut action);
                    ui.add_space(6.0);
                }
            });

        // Prompt.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(PAD);
            let (footer, _) = ui.allocate_exact_size(vec2(width, 14.0), Sense::hover());
            {
                let p = ui.painter();
                let version = crate::update::current_version();
                let short = version
                    .rsplit_once('.')
                    .map_or(version.clone(), |(v, _)| v.into());
                let right = p.layout_no_wrap(
                    format!("ondera-cli {short} · mcp"),
                    mono_font(FS_CAPS),
                    FAINT,
                );
                let right_x = footer.right() - PAD - right.size().x;
                p.galley(
                    pos2(right_x, footer.center().y - right.size().y / 2.0),
                    right.clone(),
                    FAINT,
                );
                p.with_clip_rect(Rect::from_min_max(
                    footer.min,
                    pos2(right_x - 10.0, footer.bottom()),
                ))
                .text(
                    pos2(footer.left() + PAD, footer.center().y),
                    Align2::LEFT_CENTER,
                    "⌘↵ send · ⌘Z reverts last agent change",
                    mono_font(FS_CAPS),
                    FAINT,
                );
            }
            ui.add_space(8.0);
            let (well, _) = ui.allocate_exact_size(vec2(width, input_h), Sense::hover());
            let well = well.shrink2(vec2(PAD, 0.0));
            well_input(ui.painter(), well, R_CARD);
            let ready = !self.runner.prompt.trim().is_empty() && !working;
            let send_rect = Rect::from_min_size(
                pos2(well.right() - 8.0 - 30.0, well.bottom() - 8.0 - 26.0),
                vec2(30.0, 26.0),
            );
            let mut send = false;
            place(ui, send_rect, |ui| {
                ui.add_enabled_ui(ready, |ui| {
                    let response = button(
                        ui,
                        vec2(30.0, 26.0),
                        Face::lit_flag(ready),
                        R_CONTROL,
                        |p, r, ink| icon(p, r, Icon::ArrowUp, if ready { ink } else { FAINT }),
                    )
                    .on_hover_text("Send to the agent · ⌘↵");
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(Id::new("agents-task-run-rect"), response.rect)
                    });
                    send |= response.clicked();
                });
            });
            let edit_rect = Rect::from_min_max(
                well.min + vec2(12.0, 8.0),
                pos2(send_rect.left() - 8.0, well.bottom() - 8.0),
            );
            place(ui, edit_rect, |ui| {
                ui.add_enabled_ui(!working, |ui| {
                    let edit = ui.add(
                        egui::TextEdit::multiline(&mut self.runner.prompt)
                            .id(Id::new("agents-task-prompt"))
                            .frame(false)
                            .font(font(FS_INPUT, Weight::Medium))
                            .text_color(INK)
                            .hint_text(text(
                                "Ask the agent… e.g. “double the chorus”",
                                FS_INPUT,
                                Weight::Medium,
                                FAINT,
                            ))
                            .char_limit(8000)
                            .desired_width(f32::INFINITY)
                            .desired_rows(1),
                    );
                    if edit.has_focus()
                        && ui.input_mut(|i| {
                            i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                        })
                    {
                        send = true;
                    }
                });
            });
            if send && ready {
                action = Some(Action::Send);
            }
        });
        action
    }

    fn input_height(&self) -> f32 {
        let rows = self.runner.prompt.lines().count().clamp(1, 5) as f32;
        16.0 + rows * 18.0 + 8.0
    }

    fn now_card(&mut self, ui: &mut egui::Ui, width: f32, action: &mut Option<Action>) {
        ui.add_space(PAD);
        let prose = if self.runner.status.is_empty() {
            self.runner.prompt.clone()
        } else {
            self.runner.status.clone()
        };
        let inner_w = width - 2.0 * PAD - 2.0 * PAD;
        let galley =
            ui.painter()
                .layout(prose, font(FS_PROSE, Weight::Medium), INK_BRIGHT, inner_w);
        let h = 14.0 + FS_CAPS + 6.0 + galley.size().y + 10.0 + 22.0 + 12.0;
        let (card, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
        let card = card.shrink2(vec2(PAD, 0.0));
        let p = ui.painter();
        accent_card(p, card, R_CARD);
        let x = card.left() + PAD;
        let mut y = card.top() + PAD;
        p.text(
            pos2(x, y),
            Align2::LEFT_TOP,
            "NOW",
            font(FS_CAPS, Weight::Bold),
            ACCENT,
        );
        y += FS_CAPS + 6.0;
        p.galley(pos2(x, y), galley.clone(), INK_BRIGHT);
        y += galley.size().y + 10.0;
        let row = Rect::from_min_max(pos2(x, y), pos2(card.right() - PAD, y + 22.0));
        let (elapsed, edits) = self
            .task
            .as_ref()
            .map_or((0, 0), |t| (t.started.elapsed().as_secs(), t.edits));
        let count = format!("{} edits · {}:{:02}", edits, elapsed / 60, elapsed % 60);
        let count_galley = p.layout_no_wrap(count, mono_font(FS_SMALL), DIM);
        place(ui, row, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                if small_button(ui, "Stop", Face::Raised)
                    .on_hover_text("Stop the task; finished edits stay in Undo")
                    .clicked()
                {
                    *action = Some(Action::StopTask);
                }
                let (label, _) =
                    ui.allocate_exact_size(vec2(count_galley.size().x, 22.0), Sense::hover());
                ui.painter().galley(
                    pos2(label.left(), label.center().y - count_galley.size().y / 2.0),
                    count_galley.clone(),
                    DIM,
                );
                let (bar, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
                let rail = Rect::from_center_size(bar.center(), vec2(bar.width(), 4.0));
                let p = ui.painter();
                groove(p, rail, 2.0);
                // Indeterminate sweep: no note count is known ahead of time.
                let t = (ui.input(|i| i.time) % 2.4) as f32 / 2.4;
                let span = rail.width() * 0.32;
                let x0 = rail.left() - span + t * (rail.width() + span);
                let sweep = Rect::from_min_max(
                    pos2(x0.max(rail.left()), rail.top()),
                    pos2((x0 + span).min(rail.right()), rail.bottom()),
                );
                if sweep.width() > 0.0 {
                    p.rect_filled(sweep.expand2(vec2(0.0, 2.0)), 3.0, accent(0.25));
                    p.rect_filled(sweep, 2.0, ACCENT);
                }
            });
        });
    }

    fn reply_card(&mut self, ui: &mut egui::Ui, width: f32) {
        ui.add_space(PAD);
        let (label, body, color) = match &self.runner.error {
            Some(error) => ("FAILED", error.clone(), INK),
            None => ("REPLY", self.runner.response.clone(), INK_BRIGHT),
        };
        let inner_w = width - 4.0 * PAD;
        let galley = ui
            .painter()
            .layout(body, font(FS_PROSE, Weight::Medium), color, inner_w);
        let body_h = galley.size().y.min(150.0);
        let h = 14.0 + FS_CAPS + 6.0 + body_h + 12.0;
        let (card, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
        let card = card.shrink2(vec2(PAD, 0.0));
        let p = ui.painter();
        log_entry(p, card, R_CARD, false);
        p.text(
            pos2(card.left() + PAD, card.top() + PAD),
            Align2::LEFT_TOP,
            label,
            font(FS_CAPS, Weight::Bold),
            if self.runner.error.is_some() {
                ACCENT
            } else {
                FAINT
            },
        );
        let body_rect = Rect::from_min_size(
            pos2(card.left() + PAD, card.top() + PAD + FS_CAPS + 6.0),
            vec2(inner_w, body_h),
        );
        place(ui, body_rect, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("agent-reply")
                .max_height(body_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(egui::Label::new(egui::WidgetText::from(galley.clone())).wrap());
                });
        });
    }
}

fn entry_row(
    ui: &mut egui::Ui,
    width: f32,
    entry: &mut Activity,
    undo_depth: usize,
    action: &mut Option<Action>,
) {
    let applied = entry.applied(undo_depth);
    let reverted = entry.mutated() && !applied;
    let output_galley = entry.expanded.then(|| {
        ui.painter().layout(
            format!(
                "{}\nrevision {} → {}",
                entry.output, entry.before, entry.after
            ),
            mono_font(FS_SMALL),
            DIM,
            width - 2.0 * PAD - 20.0,
        )
    });
    let h = ENTRY_H + output_galley.as_ref().map_or(0.0, |g| g.size().y + 8.0);
    let (row, response) = ui.allocate_exact_size(vec2(width, h), Sense::click());
    let row = row.shrink2(vec2(PAD, 0.0));
    let p = ui.painter();
    log_entry(p, row, R_LG, entry.running);
    let fade = if reverted { 0.55 } else { 1.0 };
    let chip = Rect::from_min_size(pos2(row.left() + 10.0, row.top() + 9.0), Vec2::splat(CHIP));
    log_chip(p, chip, entry.running);
    swatch(
        p,
        Rect::from_center_size(chip.center(), Vec2::splat(8.0)),
        entry.color.gamma_multiply(fade),
    );
    let button_label = if entry.running {
        None
    } else if entry.mutated() {
        Some(if applied { "Revert" } else { "Redo" })
    } else {
        None
    };
    let button_w = button_label.map_or(0.0, |l| {
        p.layout_no_wrap(l.into(), font(FS_VALUE, Weight::SemiBold), INK)
            .size()
            .x
            + 18.0
            + 10.0
    });
    let text_left = chip.right() + 10.0;
    let text_right = row.right() - 10.0 - button_w;
    let clip = p.with_clip_rect(Rect::from_min_max(
        pos2(text_left, row.top()),
        pos2(text_right, row.top() + ENTRY_H),
    ));
    let title_color = if entry.succeeded { INK } else { INK_BRIGHT }.gamma_multiply(fade);
    let title = clip.layout_no_wrap(
        entry.title.clone(),
        font(FS_BODY, Weight::SemiBold),
        title_color,
    );
    let title_pos = pos2(text_left, row.top() + 9.0);
    clip.galley(title_pos, title.clone(), title_color);
    if reverted {
        let y = title_pos.y + title.size().y / 2.0 + 0.5;
        hline(
            &clip,
            title_pos.x,
            title_pos.x + title.size().x.min(text_right - text_left),
            y,
            title_color,
        );
    }
    clip.text(
        pos2(text_left, row.top() + 9.0 + title.size().y + 3.0),
        Align2::LEFT_TOP,
        if entry.succeeded {
            entry.detail.clone()
        } else {
            entry.output.lines().next().unwrap_or_default().to_string()
        },
        mono_font(FS_SMALL),
        if entry.succeeded { FAINT } else { INK_DIM }.gamma_multiply(fade),
    );
    if let Some(galley) = &output_galley {
        p.galley(pos2(text_left, row.top() + ENTRY_H), galley.clone(), DIM);
    }
    if let Some(label) = button_label {
        let rect = Rect::from_min_size(
            pos2(row.right() - 10.0 - (button_w - 10.0), row.top() + 11.0),
            vec2(button_w - 10.0, 22.0),
        );
        place(ui, rect, |ui| {
            let face_kind = if applied { Face::Raised } else { Face::Pressed };
            let response = small_button(ui, label, face_kind).on_hover_text(if applied {
                "Undo this change and everything after it"
            } else {
                "Redo this change"
            });
            if response.clicked() {
                *action = Some(if applied {
                    Action::Revert(entry.sequence)
                } else {
                    Action::Redo(entry.sequence)
                });
            }
        });
    }
    let response = response.on_hover_text(if entry.expanded {
        "Click to hide the result"
    } else {
        "Click to show the result"
    });
    if response.clicked() {
        entry.expanded = !entry.expanded;
    }
    response.context_menu(|ui| {
        if ui.button("Copy command").clicked() {
            ui.ctx().copy_text(entry.detail.clone());
            ui.close();
        }
        if ui.button("Copy result").clicked() {
            ui.ctx().copy_text(entry.output.clone());
            ui.close();
        }
    });
}

impl Ondera {
    fn connection(&self) -> Connection {
        Connection {
            port: self.control.as_ref().map(control::wire::Server::port),
            discovery: self
                .control
                .as_ref()
                .map_or_else(control::wire::discovery_path, |server| {
                    server.path().to_path_buf()
                }),
        }
    }

    /// The agent panel at the right edge of the window, or its collapsed rail.
    pub(crate) fn agent_panel(&mut self, ctx: &egui::Context) {
        if !self.agents.open {
            let working = self.agents.working();
            egui::SidePanel::right("agent-rail")
                .exact_width(AGENT_RAIL)
                .resizable(false)
                .frame(egui::Frame::new().fill(AGENT_PANEL_BG))
                .show(ctx, |ui| {
                    let rect = ui.max_rect();
                    let response = ui
                        .interact(rect, ui.id().with("open"), Sense::click())
                        .on_hover_text("Show the agent panel");
                    let p = ui.painter();
                    vline(p, rect.left(), rect.top(), rect.bottom(), black(0.65));
                    if response.hovered() {
                        p.rect_filled(rect, 0.0, white(0.03));
                    }
                    accent_dot(p, pos2(rect.center().x, rect.top() + 18.0), 4.0, working);
                    vertical_caps(
                        p,
                        pos2(rect.center().x + 6.0, rect.top() + 34.0),
                        if working { "Agent · working" } else { "Agent" },
                        DIM,
                    );
                    if response.clicked() {
                        self.agents.open = true;
                    }
                });
            return;
        }
        let connection = self.connection();
        let undo_depth = self.store.undo_depth();
        let mut action = None;
        egui::SidePanel::right("agent")
            .exact_width(AGENT_PANEL)
            .resizable(false)
            .frame(egui::Frame::new().fill(AGENT_PANEL_BG))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                {
                    let p = ui.painter();
                    inset(
                        p,
                        Rect::from_min_max(rect.min, pos2(rect.left() + 6.0, rect.bottom())),
                        0.0,
                        Side::Left,
                        6.0,
                        black(0.3),
                    );
                    vline(p, rect.left(), rect.top(), rect.bottom(), black(0.65));
                }
                action = self.agents.ui(ui, &connection, undo_depth);
            });
        match action {
            Some(Action::Send) => self.start_agent_task(ctx),
            Some(Action::StopTask) => self.agents.runner.stop(),
            Some(Action::Revert(sequence)) => self.revert_activity(sequence),
            Some(Action::Redo(sequence)) => self.redo_activity(sequence),
            None => {}
        }
    }

    fn start_agent_task(&mut self, ctx: &egui::Context) {
        if self.control.is_none() {
            self.start_control(ctx);
        }
        let connection = self.connection();
        if connection.port.is_none() {
            return;
        }
        self.agents.task = Some(Task {
            started: Instant::now(),
            edits: 0,
        });
        self.agents
            .runner
            .start(&connection.discovery, &companion("ondera-mcp"));
    }

    fn revert_activity(&mut self, sequence: u64) {
        let Some(target) = self
            .agents
            .history
            .iter()
            .find(|e| e.sequence == sequence)
            .map(|e| e.depth_before)
        else {
            return;
        };
        while self.store.undo_depth() > target && self.store.can_undo() {
            self.dispatch(Command::Undo);
        }
    }

    fn redo_activity(&mut self, sequence: u64) {
        let Some(target) = self
            .agents
            .history
            .iter()
            .find(|e| e.sequence == sequence)
            .map(|e| e.depth_after)
        else {
            return;
        };
        while self.store.undo_depth() < target && self.store.can_redo() {
            self.dispatch(Command::Redo);
        }
    }

    /// The title bar's Agent menu: the panel, the task and the local bridge.
    pub(crate) fn agent_menu(&mut self, ui: &mut egui::Ui) {
        if ui
            .button(if self.agents.open {
                "Hide agent panel"
            } else {
                "Show agent panel"
            })
            .clicked()
        {
            self.agents.open = !self.agents.open;
        }
        if ui
            .add_enabled(self.agents.working(), egui::Button::new("Stop task"))
            .clicked()
        {
            self.agents.runner.stop();
        }
        ui.separator();
        let connection = self.connection();
        if ui
            .button(if connection.port.is_some() {
                "Disable local bridge"
            } else {
                "Enable local bridge"
            })
            .clicked()
        {
            if self.control.take().is_some() {
                self.agents.runner.stop();
                self.status = "Local agent bridge disabled".into();
            } else {
                let ctx = ui.ctx().clone();
                self.start_control(&ctx);
            }
        }
        ui.label(mono(
            connection.port.map_or_else(
                || "Bridge off: MCP clients and the CLI cannot reach this window".into(),
                |port| format!("Listening · 127.0.0.1:{port}"),
            ),
            FS_SMALL,
            FAINT,
        ));
        if let Some(seen) = self.agents.last_request {
            let seconds = seen.elapsed().as_secs();
            ui.label(mono(
                if seconds < 60 {
                    format!("Last request {seconds}s ago")
                } else {
                    format!("Last request {}m ago", seconds / 60)
                },
                FS_SMALL,
                FAINT,
            ));
        }
        ui.separator();
        if ui.button("Copy MCP config").clicked() {
            ui.ctx().copy_text(mcp_config(&connection.discovery));
            self.status = "MCP configuration copied".into();
            ui.close();
        }
        if ui.button("Copy CLI check").clicked() {
            ui.ctx().copy_text(cli_check(&connection.discovery));
            self.status = "CLI command copied".into();
            ui.close();
        }
        ui.menu_button("Codex CLI", |ui| {
            ui.set_min_width(260.0);
            ui.add_enabled_ui(!self.agents.working(), |ui| {
                ui.label(text(
                    "Executable · blank finds Codex automatically",
                    FS_SMALL,
                    Weight::Medium,
                    DIM,
                ));
                ui.add(
                    egui::TextEdit::singleline(&mut self.agents.runner.executable)
                        .hint_text("/path/to/codex")
                        .desired_width(f32::INFINITY),
                );
                ui.label(text(
                    "Model · blank uses your account default",
                    FS_SMALL,
                    Weight::Medium,
                    DIM,
                ));
                ui.add(
                    egui::TextEdit::singleline(&mut self.agents.runner.model)
                        .hint_text("Default")
                        .desired_width(f32::INFINITY),
                );
            });
            ui.label(text(
                "Uses your installed Codex CLI and its signed-in account; run codex login in a terminal first. Tasks see only this window's Ondera tools.",
                FS_SMALL,
                Weight::Medium,
                FAINT,
            ));
        });
    }

    pub(crate) fn record_agent_activity(
        &mut self,
        method: &str,
        params: &Value,
        source: &str,
        revision_before: u64,
        depth_before: usize,
        result: &Result<Value>,
    ) {
        let depth_after = self.store.undo_depth();
        let (title, color) = describe(method, params, self.store.session());
        self.agents.sequence += 1;
        self.agents.last_request = Some(Instant::now());
        for entry in &mut self.agents.history {
            entry.expanded = false;
        }
        let mutated = depth_after > depth_before;
        if self.agents.runner.running() {
            if let Some(task) = self.agents.task.as_mut() {
                task.edits += usize::from(mutated);
            }
        }
        let _ = source;
        self.agents.history.push_front(Activity {
            sequence: self.agents.sequence,
            title,
            detail: cli_form(method, params),
            output: bounded(match result {
                Ok(value) => pretty(value),
                Err(message) => message.clone(),
            }),
            color,
            succeeded: result.is_ok(),
            running: result
                .as_ref()
                .is_ok_and(|value| value["status"] == "running"),
            expanded: false,
            before: revision_before,
            after: self.store.revision,
            depth_before: depth_before.min(depth_after),
            depth_after,
        });
        self.agents.history.truncate(HISTORY_LIMIT);
    }

    /// A new document has no agent history and nothing to revert.
    pub(crate) fn reset_agent_history(&mut self) {
        self.agents.history.clear();
        self.agents.task = None;
    }
}

impl AgentPanel {
    #[cfg(test)]
    pub(crate) fn mock_running_task(&mut self) -> impl FnOnce() + use<> {
        self.runner.mock_running_task()
    }

    pub(crate) fn runner_busy(&self) -> bool {
        self.runner.running()
    }

    pub(crate) fn stop_runner(&mut self) {
        self.runner.stop();
    }

    pub(crate) fn poll_runner(&mut self, ctx: &egui::Context) {
        self.runner.poll();
        if self.runner.running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(40));
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// "setSendLevel" → "set send level".
fn camel_words(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_uppercase() {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn brief(value: &Value) -> String {
    match value {
        Value::String(s) => {
            let s: String = s.chars().take(28).collect();
            format!("“{s}”")
        }
        Value::Number(n) => n
            .as_f64()
            .map(|f| {
                format!("{f:.2}")
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            })
            .unwrap_or_else(|| n.to_string()),
        Value::Bool(b) => b.to_string(),
        Value::Array(items) => format!("{} items", items.len()),
        Value::Object(_) => "{…}".into(),
        Value::Null => "null".into(),
    }
}

/// A one-line human title for a log entry and the swatch of the track it touched.
fn describe(method: &str, params: &Value, session: &Session) -> (String, Color32) {
    let (object, action) = method.split_once('.').unwrap_or((method, ""));
    let clip = params["clipId"]
        .as_str()
        .and_then(|id| session.clips.iter().find(|c| c.id == id));
    let track_id = params["trackId"]
        .as_str()
        .map(str::to_string)
        .or_else(|| clip.map(|c| c.track_id.clone()));
    let track = track_id.and_then(|id| {
        session
            .tracks
            .iter()
            .position(|t| t.id == id)
            .map(|i| (i, &session.tracks[i]))
    });
    let color = match track {
        Some((i, t)) => track_color(&t.color, i),
        None if matches!(object, "track" | "clip" | "note" | "strip" | "automation") => NEUTRAL_DOT,
        None => FAINT,
    };
    let subject = match object {
        "clip" | "note" => clip.map(|c| c.name.clone()),
        _ => None,
    }
    .or_else(|| track.map(|(_, t)| t.name.clone()));
    let value = params.as_object().and_then(|map| {
        map.get("name")
            .or_else(|| {
                map.iter()
                    .find(|(k, _)| !k.ends_with("Id") && *k != "notes")
                    .map(|(_, v)| v)
            })
            .map(brief)
    });
    let mut title = format!("{} {}", capitalize(object), camel_words(action));
    // A rename already shows its result as the value; do not repeat the name.
    if let Some(s) = subject.filter(|s| value.as_deref() != Some(&format!("“{s}”"))) {
        title.push_str(&format!(" · {s}"));
    }
    if let Some(v) = value {
        title.push_str(&format!(" · {v}"));
    }
    (title, color)
}

/// The CLI spelling of a request, for the log and the clipboard.
fn cli_form(method: &str, params: &Value) -> String {
    let mut line = format!("ondera-cli {method}");
    let mut complex = serde_json::Map::new();
    if let Some(map) = params.as_object() {
        for (k, v) in map {
            match v {
                Value::String(s) => {
                    if s.chars().any(char::is_whitespace) || s.is_empty() {
                        line.push_str(&format!(" --{k} \"{}\"", s.replace('"', "\\\"")));
                    } else {
                        line.push_str(&format!(" --{k} {s}"));
                    }
                }
                Value::Number(_) | Value::Bool(_) => line.push_str(&format!(" --{k} {v}")),
                _ => {
                    complex.insert(k.clone(), v.clone());
                }
            }
        }
    }
    if !complex.is_empty() {
        line.push_str(&format!(" --params '{}'", Value::Object(complex)));
    }
    line
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

fn bounded(text: String) -> String {
    if text.len() <= DETAIL_LIMIT {
        text
    } else {
        let mut end = DETAIL_LIMIT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        format!(
            "{}\n… Display truncated; query with the CLI for the full response.",
            &text[..end]
        )
    }
}

fn companion(name: &str) -> String {
    let executable = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    if let Some(path) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(&executable)))
    {
        if path.is_file() {
            return path.to_string_lossy().into_owned();
        }
    }
    executable
}

fn mcp_config(discovery: &Path) -> String {
    pretty(&json!({
        "mcpServers": {
            "ondera": {
                "command": companion("ondera-mcp"),
                "args": ["--live"],
                "env": { "ONDERA_CONTROL": discovery.to_string_lossy() }
            }
        }
    }))
}

fn cli_check(discovery: &Path) -> String {
    let path = discovery.to_string_lossy();
    let executable = companion("ondera-cli");
    if cfg!(windows) {
        format!(
            "$env:ONDERA_CONTROL = '{}'; & '{}' --live session.info",
            path.replace('\'', "''"),
            executable.replace('\'', "''")
        )
    } else {
        format!(
            "ONDERA_CONTROL='{}' '{}' --live session.info",
            path.replace('\'', "'\\''"),
            executable.replace('\'', "'\\''")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ondera_engine::store;

    fn press(
        ctx: &egui::Context,
        panel: &mut AgentPanel,
        connection: &Connection,
    ) -> Option<Action> {
        let input = |events| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                vec2(520.0, 1400.0),
            )),
            events,
            focused: true,
            ..Default::default()
        };
        let _ = ctx.run(input(vec![]), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                panel.ui(ui, connection, 0);
            });
        });
        let rect = ctx
            .data(|data| data.get_temp::<egui::Rect>(Id::new("agents-task-run-rect")))
            .unwrap();
        let mut action = None;
        for pressed in [true, false] {
            let _ = ctx.run(
                input(vec![
                    egui::Event::PointerMoved(rect.center()),
                    egui::Event::PointerButton {
                        pos: rect.center(),
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]),
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        action = panel.ui(ui, connection, 0);
                    });
                },
            );
        }
        action
    }

    #[test]
    fn stopping_stays_busy_until_worker_cleanup_has_completed() {
        let mut panel = AgentPanel::default();
        let complete = panel.mock_running_task();
        panel.stop_runner();
        panel.poll_runner(&egui::Context::default());
        assert!(panel.runner_busy());
        complete();
        panel.poll_runner(&egui::Context::default());
        assert!(!panel.runner_busy());
        assert!(panel.runner.status.starts_with("Stopped"));
    }

    #[test]
    fn send_needs_a_prompt_and_an_idle_runner() {
        for (prompt, running, should_send) in [
            ("Write a bass line", false, true),
            ("", false, false),
            ("Write a bass line", true, false),
        ] {
            let ctx = egui::Context::default();
            install(&ctx);
            let mut panel = AgentPanel::default();
            panel.runner.prompt = prompt.into();
            let complete = running.then(|| panel.mock_running_task());
            let connection = Connection {
                port: Some(12345),
                discovery: "/tmp/control.json".into(),
            };
            let action = press(&ctx, &mut panel, &connection);
            assert_eq!(matches!(action, Some(Action::Send)), should_send);
            if let Some(complete) = complete {
                complete();
            }
        }
    }

    #[test]
    fn copied_configuration_targets_this_window_without_copying_secrets() {
        let path = Path::new("/tmp/Ondera user's session/control.json");
        let config: Value = serde_json::from_str(&mcp_config(path)).unwrap();
        assert_eq!(config["mcpServers"]["ondera"]["args"], json!(["--live"]));
        assert_eq!(
            config["mcpServers"]["ondera"]["env"]["ONDERA_CONTROL"],
            path.to_string_lossy().as_ref()
        );
        assert!(!mcp_config(path).contains("token"));
        let check = cli_check(path);
        assert!(check.contains("--live session.info"));
        assert!(check.contains("ONDERA_CONTROL"));
    }

    #[test]
    fn activity_records_success_errors_and_shared_history() {
        let mut app = Ondera::from_session(store::demo(), None);
        let before = app.store.revision;
        let depth = app.store.undo_depth();
        let params = json!({"name":"Agent session"});
        let result = control::call(&mut app, "session.rename", &params, true);
        app.record_agent_activity("session.rename", &params, "test", before, depth, &result);
        assert!(app.agents.history[0].succeeded);
        assert!(app.agents.history[0].after > before);
        assert!(app.agents.history[0].mutated());
        assert_eq!(app.store.session().name, "Agent session");
        control::call(&mut app, "history.undo", &json!({}), true).unwrap();
        assert_ne!(app.store.session().name, "Agent session");
        let before = app.store.revision;
        let depth = app.store.undo_depth();
        let result = control::call(
            &mut app,
            "track.remove",
            &json!({"trackId":"missing"}),
            true,
        );
        app.record_agent_activity("track.remove", &json!({}), "test", before, depth, &result);
        assert!(!app.agents.history[0].succeeded);
        assert!(!app.agents.history[0].mutated());
        assert_eq!(app.store.revision, before);
        for _ in 0..HISTORY_LIMIT + 1 {
            app.record_agent_activity(
                "session.info",
                &json!({}),
                "test",
                before,
                depth,
                &Ok(json!({})),
            );
        }
        assert_eq!(app.agents.history.len(), HISTORY_LIMIT);
    }

    #[test]
    fn revert_and_redo_walk_the_undo_stack_to_the_entry() {
        let mut app = Ondera::from_session(store::demo(), None);
        let original = app.store.session().name.clone();
        for name in ["First", "Second"] {
            let before = app.store.revision;
            let depth = app.store.undo_depth();
            let params = json!({ "name": name });
            let result = control::call(&mut app, "session.rename", &params, true);
            app.record_agent_activity("session.rename", &params, "test", before, depth, &result);
        }
        let first = app.agents.history[1].sequence;
        let second = app.agents.history[0].sequence;
        app.revert_activity(first);
        assert_eq!(app.store.session().name, original);
        assert!(!app.agents.history[1].applied(app.store.undo_depth()));
        assert!(!app.agents.history[0].applied(app.store.undo_depth()));
        app.redo_activity(second);
        assert_eq!(app.store.session().name, "Second");
        assert!(app.agents.history[0].applied(app.store.undo_depth()));
    }

    #[test]
    fn titles_and_cli_form_name_the_track_and_the_value() {
        let session = store::demo();
        let track = &session.tracks[1];
        let params = json!({ "trackId": track.id, "name": "Lead Vox" });
        let (title, color) = describe("track.rename", &params, &session);
        assert_eq!(title, format!("Track rename · {} · “Lead Vox”", track.name));
        let (title, _) = describe(
            "track.rename",
            &json!({ "trackId": track.id, "name": track.name }),
            &session,
        );
        assert_eq!(title, format!("Track rename · “{}”", track.name));
        assert_eq!(color, track_color(&track.color, 1));
        let cli = cli_form("track.rename", &params);
        assert!(cli.starts_with("ondera-cli track.rename "));
        assert!(cli.contains(&format!("--trackId {}", track.id)));
        assert!(cli.contains("--name \"Lead Vox\""));
        let (title, _) = describe("session.info", &json!({}), &session);
        assert_eq!(title, "Session info");
        assert!(cli_form("clip.setNotes", &json!({"clipId":"c","notes":[]})).contains("--params"));
    }

    #[test]
    fn panel_lays_out_while_working_and_after_a_reply() {
        let connection = Connection {
            port: Some(12345),
            discovery: "/tmp/control.json".into(),
        };
        for (running, response, error) in [
            (true, "", None),
            (false, "Added a bass line over bars 1–8.", None),
            (false, "", Some("codex exited with status 1".to_string())),
        ] {
            let ctx = egui::Context::default();
            install(&ctx);
            let mut panel = AgentPanel::default();
            panel.runner.prompt = "Add a bass line".into();
            panel.runner.status = "Calling ondera clip.setNotes…".into();
            panel.runner.response = response.into();
            panel.runner.error = error;
            panel.task = Some(Task {
                started: Instant::now(),
                edits: 3,
            });
            let complete = running.then(|| panel.mock_running_task());
            let action = press(&ctx, &mut panel, &connection);
            assert!(action.is_none() == running);
            if let Some(complete) = complete {
                complete();
            }
        }
    }

    #[test]
    fn large_activity_is_bounded_on_a_utf8_boundary() {
        let output = bounded("🎹".repeat(DETAIL_LIMIT));
        assert!(output.len() < DETAIL_LIMIT + 100);
        assert!(output.contains("Display truncated"));
    }
}
