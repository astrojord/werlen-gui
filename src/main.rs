mod db;
mod shop;
mod ui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Werlen's Ware Generator")
            .with_inner_size([900.0, 640.0])
            .with_min_inner_size([520.0, 360.0]),
        ..Default::default()
    };
    eframe::run_native(
        "werlen-gui",
        options,
        Box::new(|cc| {
            ui::setup_fonts(&cc.egui_ctx);
            Ok(Box::new(ui::WerlenApp::new()))
        }),
    )
}
