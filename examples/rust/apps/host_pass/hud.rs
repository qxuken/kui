use kui_core::{Align, Color, NodeSpec, Role, TextStyle, Ui, UiEvent, Value, widgets};

use crate::game::Game;

const ROW_H: f32 = 28.0;

pub fn view(game: &Game, ui: &mut Ui<'_>) {
    let theme = ui.theme();
    let metrics = ui.metrics();
    let panel = theme.surface.with_alpha(0.92);
    let title = TextStyle::new(20.0).bold();
    let body = TextStyle::new(16.0);
    let caption = TextStyle::new(14.0).color(theme.muted);

    ui.with(
        NodeSpec::row()
            .fill()
            .main_align(Align::SpaceBetween)
            .pad(16.0),
        |ui| {
            ui.with(
                NodeSpec::column()
                    .width(260.0)
                    .gap(10.0)
                    .pad(14.0)
                    .bg(panel)
                    .radius(metrics.radius),
                |ui| {
                    ui.text("Replicator Bloom", title);
                    ui.text(&format!("Gold: {}", game.gold), body);
                    ui.text(&format!("Drones: {}", game.drones), body);
                    let energy = format!("Energy {:.0}%", game.energy);
                    ui.text(&energy, caption);
                    widgets::progress_with(
                        ui,
                        "Energy",
                        widgets::progress_spec(&metrics)
                            .grow_width()
                            .role(Role::Meter)
                            .value_now(game.energy)
                            .value_max(100.0)
                            .value_text(energy),
                        None,
                    );
                    ui.with(NodeSpec::row().gap(8.0), |ui| {
                        widgets::button_with(
                            ui,
                            "harvest",
                            "Harvest",
                            widgets::button_spec(&theme, &metrics)
                                .disabled(!game.can_harvest())
                                .on_click("harvest"),
                            None,
                        );
                        widgets::button_with(
                            ui,
                            "build",
                            "Build drone (10g)",
                            widgets::button_spec(&theme, &metrics)
                                .disabled(!game.can_build())
                                .on_click("build"),
                            None,
                        );
                    });
                    widgets::checkbox(ui, "Auto-harvest", game.auto_harvest, "auto");
                },
            );

            ui.with(
                NodeSpec::column()
                    .size(300.0, 320.0)
                    .gap(8.0)
                    .pad(14.0)
                    .bg(panel)
                    .radius(metrics.radius),
                |ui| {
                    ui.text("Log", title);
                    let slice = widgets::uniform_list_begin(
                        ui,
                        "log",
                        NodeSpec::column().fill(),
                        game.log.len(),
                        ROW_H,
                    );
                    for row in slice.rows.clone() {
                        let selected = game.selected == Some(row);
                        let spec = NodeSpec::column()
                            .grow_width()
                            .pad_xy(6.0, 3.0)
                            .radius(metrics.radius_inner)
                            .bg(if selected {
                                theme.selection
                            } else {
                                Color::TRANSPARENT
                            })
                            .hover_bg(theme.hover)
                            .selected(selected)
                            .on_click(Value::map([
                                ("kind", Value::str("row")),
                                ("row", Value::Int(row as i64)),
                            ]));
                        let line = game.log_seq - 1 - row;
                        ui.with_indexed(line as u64, slice.row_spec(spec), |ui| {
                            ui.text(&game.log[row], body);
                        });
                    }
                    widgets::uniform_list_end(ui, &slice);
                },
            );
            widgets::latency_hud_at(ui, Align::Start, Align::End);
        },
    );
}

pub fn apply(game: &mut Game, events: &[UiEvent]) {
    for event in events {
        match event.payload.get_str("tag") {
            Some("harvest") => game.harvest(),
            Some("build") => game.build(),
            Some("auto") => game.toggle_auto(),
            _ => {}
        }
        if event.payload.get_str("kind") == Some("row")
            && let Some(row) = event.payload.get_int("row")
        {
            game.select(row as usize);
        }
    }
}
