pub mod about{
    use egui::{Id, Modal};
    pub fn show_about(ui: &mut egui::Ui,is_open: &mut bool){
        Modal::new(Id::new("Modal b")).show(ui.ctx(), |ui| {
            ui.set_width(500.0);

            ui.heading("boltshell  0.1.0");

            ui.separator();

            ui.vertical(|ui|{
                ui.label("egui版本: 0.32.3");
                ui.label("rust版本: 1.90.0");
            });
            ui.add(egui::github_link_file_line!("https://github.com/fengzhongsikao/boltshell", "项目github地址"));
            ui.add(egui::github_link_file_line!("https://github.com/TheBlindM/T-Shell", "借鉴T-Shell"));
            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui.button("退出").clicked(){
                        *is_open = false;
                    };
                },
            );
        });
    }
}