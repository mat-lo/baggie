mod app;
mod bagit;

use app::BagItApp;
use eframe::icon_data::from_png_bytes;

fn main() -> eframe::Result<()> {
    let icon = from_png_bytes(include_bytes!("../icon.png")).expect("Failed to load icon");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 230.0])
            .with_min_inner_size([300.0, 250.0])
            .with_drag_and_drop(true)
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "Baggie",
        options,
        Box::new(|cc| Ok(Box::new(BagItApp::new(cc)))),
    )
}
