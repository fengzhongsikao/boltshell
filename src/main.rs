// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use boltshell::models::home::home;
use eframe::egui;
use eframe::egui::IconData;
use image;
use std::sync::Arc;
use boltshell::models::database::sqlite;
#[tokio::main]
async fn main() {
    let mut native_options = eframe::NativeOptions::default();
    let icon_data = include_bytes!("../data/bolt.png");
    let img = image::load_from_memory_with_format(icon_data, image::ImageFormat::Png).unwrap();
    let rgba_data = img.into_rgba8();
    let (w, h) = (rgba_data.width(), rgba_data.height());
    let raw_data: Vec<u8> = rgba_data.into_raw();

    let db_manager = Arc::new(
        sqlite::DatabaseManager::new("sessions.db")
            .expect("Failed to initialize database")
    );

    native_options.viewport = egui::ViewportBuilder::default()
        .with_inner_size([1200.0, 700.0]) // 设置初始窗口大小
        .with_min_inner_size([1000.0, 700.0]); // 设置最小窗口大小
    native_options.viewport.icon = Some(Arc::<IconData>::new(IconData {
        rgba: raw_data,
        width: w,
        height: h,
    }));
    let _ = eframe::run_native(
        "boltshell",
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            Ok(Box::new(home::MyEguiApp::new(cc,db_manager)))
        }),
    );
}
