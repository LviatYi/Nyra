use crate::job::entity::{ScreenPoint, ScreenRect};
use crate::overlay::asset::install_fonts;
use crate::overlay::selection::SelectionPhase;
use crate::overlay::{OverlayMode, OverlayState};
use crate::perception::perception_service::service::{
    PerceptionRequest, PerceptionResponse, PerceptionService,
};
use crate::platform::{OverlayController, OverlayPlatformEvent};
use eframe::egui::{
    self, Align2, Color32, FontId, Frame, Id, LayerId, Order, Painter, Pos2, Rect, Sense, Stroke,
    StrokeKind, Vec2,
};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

pub struct NyraOverlayApp {
    state: OverlayState,
    tasks: Vec<OverlayTask>,
    next_task_id: u64,
    next_request_id: u64,
    selected_task_id: Option<u64>,
    pending_task_upsert: Option<PendingTaskUpsert>,
    result_tx: Sender<TaskExecutionMessage>,
    result_rx: Receiver<TaskExecutionMessage>,
    perception_service: PerceptionService,
    fonts_loaded: bool,
    platform: OverlayController,
}

#[derive(Debug, Clone)]
struct OverlayTask {
    id: u64,
    title: String,
    region: ScreenRect,
    frequency: Duration,
    enabled: bool,
    last_run_at: Option<Instant>,
    last_result_at: Option<Instant>,
    last_request_id: Option<u64>,
    status: TaskStatus,
}

#[derive(Debug, Clone)]
enum TaskStatus {
    Idle,
    Running,
    Success { summary: String, text: String },
    Failed(String),
}

#[derive(Debug, Clone)]
struct PendingTaskUpsert {
    task_id: Option<u64>,
    region: ScreenRect,
    title: String,
    frequency_secs: f32,
    enabled: bool,
}

#[derive(Debug)]
struct TaskExecutionMessage {
    task_id: u64,
    request_id: u64,
    finished_at: Instant,
    response: PerceptionResponse,
}

impl NyraOverlayApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        title: &str,
        perception_service: PerceptionService,
    ) -> Self {
        cc.egui_ctx.set_pixels_per_point(1.0);
        let (result_tx, result_rx) = mpsc::channel();

        Self {
            state: OverlayState::default(),
            tasks: Vec::new(),
            next_task_id: 1,
            next_request_id: 1,
            selected_task_id: None,
            pending_task_upsert: None,
            result_tx,
            result_rx,
            perception_service,
            fonts_loaded: false,
            platform: OverlayController::new(title),
        }
    }

    fn ensure_fonts(&mut self, ctx: &egui::Context) {
        if self.fonts_loaded {
            return;
        }

        install_fonts(ctx);
        self.fonts_loaded = true;
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        for event in self.platform.poll_events() {
            match event {
                OverlayPlatformEvent::ToggleMode => {
                    self.state.mode = self.state.mode.toggle();
                }
                OverlayPlatformEvent::ClearSelection => {
                    self.state.selection = SelectionPhase::Idle;
                    self.pending_task_upsert = None;
                }
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F8)) {
            self.state.mode = self.state.mode.toggle();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.state.selection = SelectionPhase::Idle;
            self.pending_task_upsert = None;
        }
    }

    fn handle_selection_canvas(&mut self, ui: &mut egui::Ui, canvas_rect: Rect) {
        let response = ui.interact(
            canvas_rect,
            Id::new("overlay-canvas"),
            Sense::click_and_drag(),
        );

        if self.state.mode == OverlayMode::Sentinel {
            return;
        }

        if response.drag_started() {
            if let Some(pointer) = response.interact_pointer_pos() {
                self.state.selection = SelectionPhase::start_at(ScreenPoint {
                    x: pointer.x.round() as i32,
                    y: pointer.y.round() as i32,
                });
            }
        }

        if response.dragged() {
            if let Some(pointer) = response.interact_pointer_pos() {
                self.state.selection = self.state.selection.update_by(ScreenPoint {
                    x: pointer.x.round() as i32,
                    y: pointer.y.round() as i32,
                });
            }
        }

        if response.drag_stopped() {
            self.finish_selection();
        }
    }

    fn finish_selection(&mut self) {
        let preview = match self.state.selection {
            SelectionPhase::Dragging(dragging) => dragging.confirm(),
            SelectionPhase::Preview(preview) => preview,
            _ => return,
        };

        let region = preview.region();
        if !is_large_enough(region) {
            self.state.selection = SelectionPhase::Idle;
            return;
        }

        let task_id = self.selected_task_id;
        let template = self.task_by_id(task_id);
        self.pending_task_upsert = Some(PendingTaskUpsert {
            task_id,
            region,
            title: template
                .map(|task| task.title.clone())
                .unwrap_or_else(|| format!("Task {}", self.next_task_id)),
            frequency_secs: template
                .map(|task| task.frequency.as_secs_f32())
                .unwrap_or(3.0),
            enabled: template.map(|task| task.enabled).unwrap_or(true),
        });
        self.state.selection = SelectionPhase::Preview(preview);
    }

    fn sync_task_results(&mut self) {
        while let Ok(message) = self.result_rx.try_recv() {
            let Some(task) = self.tasks.iter_mut().find(|task| task.id == message.task_id) else {
                continue;
            };

            task.last_result_at = Some(message.finished_at);
            task.last_request_id = Some(message.request_id);

            match message.response {
                Ok(result) => {
                    task.region = result.region;
                    task.status = TaskStatus::Success {
                        summary: result.summary,
                        text: result.text,
                    };
                }
                Err(error) => {
                    task.status = TaskStatus::Failed(error.message);
                }
            }
        }
    }

    fn schedule_due_tasks(&mut self) {
        let now = Instant::now();
        let due_task_ids: Vec<u64> = self
            .tasks
            .iter()
            .filter(|task| task.enabled && !matches!(task.status, TaskStatus::Running))
            .filter(|task| {
                task.last_run_at
                    .map(|last_run| now.duration_since(last_run) >= task.frequency)
                    .unwrap_or(true)
            })
            .map(|task| task.id)
            .collect();

        for task_id in due_task_ids {
            self.spawn_task_run(task_id, now);
        }
    }

    fn spawn_task_run(&mut self, task_id: u64, now: Instant) {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == task_id) else {
            return;
        };

        let request_id = self.next_request_id;
        self.next_request_id += 1;

        task.last_run_at = Some(now);
        task.last_request_id = Some(request_id);
        task.status = TaskStatus::Running;

        let service = self.perception_service.clone();
        let tx = self.result_tx.clone();
        let region = task.region;

        thread::Builder::new()
            .name(format!("nyra-overlay-task-{task_id}"))
            .spawn(move || {
                let response = service.sync_analyze(PerceptionRequest {
                    request_id,
                    region,
                    submitted_at: Instant::now(),
                });

                let _ = tx.send(TaskExecutionMessage {
                    task_id,
                    request_id,
                    finished_at: Instant::now(),
                    response,
                });
            })
            .expect("failed to spawn overlay task");
    }

    fn draw_toolbar(&mut self, ctx: &egui::Context) {
        egui::Window::new("Nyra Control")
            .title_bar(true)
            .collapsible(false)
            .default_width(360.0)
            .default_pos(Pos2::new(24.0, 24.0))
            .frame(
                Frame::window(&ctx.style())
                    .fill(Color32::from_rgba_unmultiplied(18, 22, 30, 236))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(70, 90, 120))),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("模式");
                    if ui.button(mode_label(self.state.mode)).clicked() {
                        self.state.mode = self.state.mode.toggle();
                    }
                    if ui.button("新建任务").clicked() {
                        self.selected_task_id = None;
                        self.pending_task_upsert = None;
                        self.state.selection = SelectionPhase::Idle;
                    }
                });

                ui.small("F8 切换穿透；在 Edit 模式拖拽屏幕区域后，可新增或更新任务。");
                ui.separator();

                self.draw_pending_editor(ui);

                ui.separator();
                ui.heading("任务");

                if self.tasks.is_empty() {
                    ui.small("暂无任务。先拖拽一个区域，再在上方创建任务。");
                }

                let task_ids: Vec<u64> = self.tasks.iter().map(|task| task.id).collect();
                for task_id in task_ids {
                    self.draw_task_card(ui, task_id);
                }
            });
    }

    fn draw_pending_editor(&mut self, ui: &mut egui::Ui) {
        ui.heading("当前框选");

        match self.pending_task_upsert.as_mut() {
            Some(pending) => {
                let mut submit_clicked = false;
                let mut cancel_clicked = false;
                ui.monospace(format_region(pending.region));
                ui.horizontal(|ui| {
                    ui.label("名称");
                    ui.text_edit_singleline(&mut pending.title);
                });
                ui.add(
                    egui::Slider::new(&mut pending.frequency_secs, 0.5..=60.0)
                        .logarithmic(true)
                        .suffix(" s")
                        .text("频率"),
                );
                ui.checkbox(&mut pending.enabled, "启用任务");

                ui.horizontal(|ui| {
                    let button_label = if pending.task_id.is_some() {
                        "更新任务"
                    } else {
                        "创建任务"
                    };
                    if ui.button(button_label).clicked() {
                        submit_clicked = true;
                    }

                    if ui.button("取消").clicked() {
                        cancel_clicked = true;
                    }
                });

                if submit_clicked {
                    let task_id = pending.task_id;
                    let title = pending.title.clone();
                    let region = pending.region;
                    let frequency = Duration::from_secs_f32(pending.frequency_secs.max(0.5));
                    let enabled = pending.enabled;
                    self.upsert_task(task_id, title, region, frequency, enabled);
                }

                if cancel_clicked {
                    self.pending_task_upsert = None;
                    self.state.selection = SelectionPhase::Idle;
                }
            }
            None => {
                ui.small("拖拽屏幕区域以创建任务，或选择已有任务后重新框选来更新区域。");
            }
        }
    }

    fn draw_task_card(&mut self, ui: &mut egui::Ui, task_id: u64) {
        let Some(index) = self.tasks.iter().position(|task| task.id == task_id) else {
            return;
        };

        let mut delete_clicked = false;
        let mut run_now_clicked = false;
        let mut edit_region_clicked = false;

        let selected = self.selected_task_id == Some(task_id);
        let frame = Frame::group(ui.style()).fill(if selected {
            Color32::from_rgba_unmultiplied(36, 48, 64, 220)
        } else {
            Color32::from_rgba_unmultiplied(28, 34, 44, 190)
        });

        frame.show(ui, |ui| {
            let task = &mut self.tasks[index];

            ui.horizontal(|ui| {
                if ui
                    .selectable_label(selected, format!("#{} {}", task.id, task.title))
                    .clicked()
                {
                    self.selected_task_id = Some(task.id);
                }

                ui.checkbox(&mut task.enabled, "启用");
            });

            ui.monospace(format_region(task.region));
            ui.label(format!("频率: {:.1}s", task.frequency.as_secs_f32()));
            ui.label(format!("状态: {}", task.status_line()));

            if let Some(last_result_at) = task.last_result_at {
                ui.small(format!("最近完成: {:.1}s 前", last_result_at.elapsed().as_secs_f32()));
            }

            if let Some(last_request_id) = task.last_request_id {
                ui.small(format!("最近请求: #{last_request_id}"));
            }

            ui.horizontal(|ui| {
                if ui.button("立即执行").clicked() {
                    run_now_clicked = true;
                }
                if ui.button("更新区域").clicked() {
                    edit_region_clicked = true;
                }
                if ui.button("删除").clicked() {
                    delete_clicked = true;
                }
            });
        });

        if run_now_clicked {
            self.spawn_task_run(task_id, Instant::now());
        }

        if edit_region_clicked {
            self.selected_task_id = Some(task_id);
            self.pending_task_upsert = None;
            self.state.selection = SelectionPhase::Idle;
        }

        if delete_clicked {
            self.tasks.remove(index);
            if self.selected_task_id == Some(task_id) {
                self.selected_task_id = None;
            }
            if self.pending_task_upsert.as_ref().and_then(|pending| pending.task_id) == Some(task_id)
            {
                self.pending_task_upsert = None;
            }
        }
    }

    fn upsert_task(
        &mut self,
        task_id: Option<u64>,
        title: String,
        region: ScreenRect,
        frequency: Duration,
        enabled: bool,
    ) {
        let title = if title.trim().is_empty() {
            format!("Task {}", task_id.unwrap_or(self.next_task_id))
        } else {
            title.trim().to_string()
        };

        if let Some(task_id) = task_id {
            if let Some(task) = self.tasks.iter_mut().find(|task| task.id == task_id) {
                task.title = title;
                task.region = region;
                task.frequency = frequency;
                task.enabled = enabled;
            }
            self.selected_task_id = Some(task_id);
        } else {
            let new_task_id = self.next_task_id;
            self.next_task_id += 1;
            self.selected_task_id = Some(new_task_id);
            self.tasks.push(OverlayTask {
                id: new_task_id,
                title,
                region,
                frequency,
                enabled,
                last_run_at: None,
                last_result_at: None,
                last_request_id: None,
                status: TaskStatus::Idle,
            });
        }

        self.pending_task_upsert = None;
        self.state.selection = SelectionPhase::Idle;
    }

    fn draw_overlay(&self, ctx: &egui::Context) {
        let painter =
            ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("overlay-painter")));

        for task in &self.tasks {
            draw_task_region(&painter, task, self.selected_task_id == Some(task.id));
        }

        if let Some(region) = selection_rect(self.state.selection) {
            draw_selection_preview(&painter, region);
        }
    }

    fn task_by_id(&self, task_id: Option<u64>) -> Option<&OverlayTask> {
        let task_id = task_id?;
        self.tasks.iter().find(|task| task.id == task_id)
    }
}

impl eframe::App for NyraOverlayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_fonts(ctx);
        self.handle_shortcuts(ctx);
        self.sync_task_results();
        self.schedule_due_tasks();
        self.platform.apply_mode(self.state.mode);

        egui::CentralPanel::default()
            .frame(Frame::default().fill(Color32::TRANSPARENT))
            .show(ctx, |ui| {
                let canvas_rect = ui.max_rect();
                self.handle_selection_canvas(ui, canvas_rect);
            });

        self.draw_toolbar(ctx);
        self.draw_overlay(ctx);
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        Color32::TRANSPARENT.to_normalized_gamma_f32()
    }
}

impl OverlayTask {
    fn status_line(&self) -> String {
        match &self.status {
            TaskStatus::Idle => "idle".to_string(),
            TaskStatus::Running => "running".to_string(),
            TaskStatus::Success { summary, .. } => summary.clone(),
            TaskStatus::Failed(message) => format!("failed: {message}"),
        }
    }
}

fn selection_rect(selection: SelectionPhase) -> Option<ScreenRect> {
    match selection {
        SelectionPhase::Idle => None,
        SelectionPhase::Start(start) => Some(ScreenRect::from_points(start.point(), start.point())),
        SelectionPhase::Dragging(dragging) => Some(dragging.get_region()),
        SelectionPhase::Preview(preview) => Some(preview.region()),
    }
}

fn is_large_enough(region: ScreenRect) -> bool {
    region.width() >= 8 && region.height() >= 8
}

fn format_region(region: ScreenRect) -> String {
    format!(
        "区域: ({}, {}) {} x {}",
        region.lt_x(),
        region.lt_y(),
        region.width(),
        region.height()
    )
}

fn mode_label(mode: OverlayMode) -> &'static str {
    match mode {
        OverlayMode::Edit => "Edit",
        OverlayMode::Sentinel => "Sentinel",
    }
}

fn draw_selection_preview(painter: &Painter, rect: ScreenRect) {
    draw_rect_outline(
        painter,
        rect,
        Color32::from_rgba_unmultiplied(64, 200, 140, 28),
        Color32::from_rgb(92, 255, 174),
        "新框选",
    );
}

fn draw_task_region(painter: &Painter, task: &OverlayTask, selected: bool) {
    let (fill, stroke) = match (&task.status, selected) {
        (TaskStatus::Running, _) => (
            Color32::from_rgba_unmultiplied(255, 196, 92, 28),
            Color32::from_rgb(255, 196, 92),
        ),
        (TaskStatus::Failed(_), _) => (
            Color32::from_rgba_unmultiplied(255, 96, 96, 24),
            Color32::from_rgb(255, 120, 120),
        ),
        (_, true) => (
            Color32::from_rgba_unmultiplied(92, 160, 255, 24),
            Color32::from_rgb(120, 190, 255),
        ),
        _ => (
            Color32::from_rgba_unmultiplied(64, 200, 140, 18),
            Color32::from_rgb(92, 255, 174),
        ),
    };

    draw_rect_outline(painter, task.region, fill, stroke, &task.title);

    let badge_rect = to_egui_rect(task.region);
    let status_text = match &task.status {
        TaskStatus::Idle => "idle".to_string(),
        TaskStatus::Running => "running".to_string(),
        TaskStatus::Success { text, .. } => text.clone(),
        TaskStatus::Failed(message) => message.clone(),
    };

    painter.text(
        badge_rect.left_top() + Vec2::new(8.0, 8.0),
        Align2::LEFT_TOP,
        status_text,
        FontId::proportional(14.0),
        Color32::WHITE,
    );
}

fn draw_rect_outline(
    painter: &Painter,
    rect: ScreenRect,
    fill: Color32,
    stroke: Color32,
    title: &str,
) {
    let egui_rect = to_egui_rect(rect);

    painter.rect_filled(egui_rect, 4.0, fill);
    painter.rect_stroke(
        egui_rect,
        4.0,
        Stroke::new(2.0, stroke),
        StrokeKind::Middle,
    );
    painter.text(
        egui_rect.left_top() + Vec2::new(0.0, -22.0),
        Align2::LEFT_TOP,
        format!("{title}  {}x{}", rect.width(), rect.height()),
        FontId::proportional(15.0),
        Color32::WHITE,
    );
}

fn to_egui_rect(rect: ScreenRect) -> Rect {
    Rect::from_min_size(
        Pos2::new(rect.lt_x() as f32, rect.lt_y() as f32),
        Vec2::new(rect.width() as f32, rect.height() as f32),
    )
}
