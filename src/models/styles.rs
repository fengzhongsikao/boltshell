pub mod style {
    pub fn load_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "my_font".to_owned(),
            egui::FontData::from_static(include_bytes!("../../fonts/LXGWWenKai-Medium.ttf")).into(),
        );
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "my_font".to_owned());
        fonts
            .families
            .get_mut(&egui::FontFamily::Monospace)
            .unwrap()
            .push("my_font".to_owned());
        ctx.set_fonts(fonts);
    }

    pub fn hover_icon_with_bg(
        ui: &mut egui::Ui,
        image: egui::ImageSource,
        size: egui::Vec2,
    ) -> egui::Response {
        let padding = 8.0;
        let desired_size = egui::Vec2::new(size.x + padding * 2.0, size.y + padding * 2.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            // 绘制背景
            if response.hovered() {
                ui.painter().rect_filled(
                    rect,
                    4.0,                                                       // 圆角
                    egui::Color32::from_rgba_premultiplied(187, 189, 201, 50), // 半透明背景
                );
            }

            // 绘制图标
            let image_rect = egui::Rect::from_center_size(rect.center(), size);
            let tint = if response.hovered() {
                egui::Color32::from_rgb(80, 97, 187) // 悬停时图标颜色
            } else {
                egui::Color32::WHITE
            };

            egui::Image::new(image).tint(tint).paint_at(ui, image_rect);
        }
        response
    }
}
