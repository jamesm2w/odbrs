use eframe::{egui::{Context, Window, Frame, Sense}, epaint::{vec2, Shape, Stroke, Color32}};

use super::App;

pub fn render_map(app_state: &mut App, ctx: &Context, _frame: &mut eframe::Frame) {
    Window::new("Simulation Map").default_size(vec2(800.0, 600.0))
        .frame(Frame::window(&ctx.style())
            .fill(Color32::GRAY)
        )
        .show(ctx, |ui| {
        
        let (mut response, mut painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        
        app_state.graph.view(&mut response, &mut painter, ui);

        let transform = app_state.graph.get_transform().read().unwrap();

        painter.extend(app_state.state.borrow().agent_display_data.iter().map(|shp| {
            transform.map_shape_to_screen(shp.clone())
        }).collect::<Vec<_>>());

        // Draw demand data?
        if let Some(demand_gen) = &app_state.state.borrow().demand_gen {
            painter.extend(
                demand_gen
                    .get_demand_queue()
                    .read()
                    .expect("GUI Couldn't read demand_gen")
                    .iter()
                    .map(|demand| {
                        Shape::Vec(vec![
                            // Shape::line_segment([
                            //     self.graph.get_transform().read().unwrap().map_to_screen(demand.0.0 as _, demand.0.1 as _),
                            //     self.graph.get_transform().read().unwrap().map_to_screen(demand.1.0 as _, demand.1.1 as _)
                            // ], Stroke::new(1.5, Color32::LIGHT_GREEN)),
                            Shape::circle_stroke(
                                app_state.graph
                                    .get_transform()
                                    .read()
                                    .unwrap()
                                    .map_to_screen(demand.0 .0 as _, demand.0 .1 as _),
                                1.0,
                                Stroke::new(1.5, Color32::LIGHT_GREEN),
                            ),
                            Shape::circle_stroke(
                                app_state.graph
                                    .get_transform()
                                    .read()
                                    .unwrap()
                                    .map_to_screen(demand.1 .0 as _, demand.1 .1 as _),
                                1.0,
                                Stroke::new(1.5, Color32::LIGHT_RED),
                            ),
                            //TODO: tidy up this lol
                        ])
                    })
                    .collect::<Vec<_>>(),
            )
        }
    });
}