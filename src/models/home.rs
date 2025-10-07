pub mod home {
    use crate::models::about::about;
    use crate::models::database::sqlite;
    use crate::models::styles::style;
    use egui::{CentralPanel, Id, Modal};
    use egui_extras::{Column, TableBuilder};
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, mpsc};
    use std::thread;
    use ssh2::Session;
    use tokio::runtime::Handle;
    use tokio::task;
    // 新增导入
    #[derive(Default, PartialEq)]
    enum Page {
        #[default]
        Home,
        Settings,
        Terminal(String), // 新增：保存 session name
    }

    #[derive(Default)]
    enum Cmd {
        #[default]
        None,
        Test {
            host: String,
            port: String,
            user: String,
            pwd:  String,
            style: CertStyle,
        },
    }

    #[derive(Debug,Clone, PartialEq)]
    enum CertStyle {
        First,
        Second,
    }
    #[derive(Debug, PartialEq)]
    enum ConnetStyle {
        直连,
        Socket,
        Http,
        Jumphost,
    }

    impl Default for ConnetStyle {
        fn default() -> Self {
            Self::直连 // 指定一个默认值
        }
    }

    impl Default for CertStyle {
        fn default() -> Self {
            Self::First // 指定一个默认值
        }
    }
    struct Tab {
        label: String,
        page: Page,
        closable: bool,
    }

    pub struct MyEguiApp {
        show_add_dialog: bool,
        show_about_dialog: bool,
        page: Page,
        name: String,
        group_name: String,
        id: i32,
        ip: String,
        port: String,
        user_name: String,
        password: String,
        select_button: CertStyle,
        select_connect: ConnetStyle,
        picked_path: Option<String>,
        is_loading: bool,
        error_message: Option<String>,
        test_message: Option<String>,
        db_manager: Arc<sqlite::DatabaseManager>,
        rx: Option<mpsc::Receiver<rusqlite::Result<i32>>>,
        handle: Option<task::JoinHandle<()>>,
        should_save: bool,
        show_editor: bool,
        should_update: bool,
        sessions: Vec<sqlite::Session>,
        pub test_rx: Option<mpsc::Receiver<Result<String, String>>>,
        tabs: Vec<Tab>,
        active_tab: usize,
        terminal_input: String,
        terminal_input2: String,
        terminal_output: Vec<String>,
        terminal_tx: Option<mpsc::Sender<String>>,
        terminal_rx: Option<mpsc::Receiver<String>>,
        remote_prompt: String,      // 远端提示符，例如 "user@host:~$ "
        first_banner_arrived: bool, // 是否已收到第一条 banner
        terminal_should_stop: Arc<AtomicBool>, // 新增
        terminal_thread_handle: Option<thread::JoinHandle<()>>, // 新增
        cmd: Cmd,

    }

    impl MyEguiApp {
        pub fn new(
            cc: &eframe::CreationContext<'_>,
            db_manager: Arc<sqlite::DatabaseManager>,
        ) -> Self {
            style::load_fonts(&cc.egui_ctx);

            let sessions1 =
                task::block_in_place(|| Handle::current().block_on(db_manager.get_sessions()))
                    .unwrap_or_default();
            for (i, s) in sessions1.iter().enumerate() {
                println!(
                    "[{}] id={} name={}  group={}  {}:{}",
                    i, s.id, s.name, s.group_name, s.ip, s.port
                );
            }
            Self {
                show_add_dialog: false,
                show_about_dialog: false,
                page: Page::Home,
                name: "".to_string(),
                group_name: "".to_string(),
                id:0,
                ip: "".to_string(),
                port: "".to_string(),
                user_name: "".to_string(),
                password: "".to_string(),
                select_button: CertStyle::First,
                select_connect: ConnetStyle::直连,
                picked_path: None,
                is_loading: false,
                error_message: None,
                db_manager,
                handle: None,
                rx: None,
                should_save: false,
                show_editor: false,
                should_update: false,
                sessions: sessions1,
                test_message: None,
                test_rx: None,
                tabs: vec![Tab {
                    label: "首页".to_string(),
                    page: Page::Home,
                    closable: false,
                }],
                active_tab: 0,
                terminal_input: "".to_string(),
                terminal_input2: "".to_string(),
                terminal_output: Vec::new(),
                terminal_tx: None,
                terminal_rx: None,
                remote_prompt: "".to_string(), // 远端提示符，例如 "user@host:~$ "
                first_banner_arrived: false,   // 是否已收到第一条 banner
                terminal_should_stop: Arc::new(AtomicBool::new(false)), // 修正这里
                terminal_thread_handle: None,
                cmd:Cmd::None,
            }
        }

        fn show_tab_bar(&mut self, ui: &mut egui::Ui) {
            ui.horizontal(|ui| {
                let mut to_remove = None;

                for (i, tab) in self.tabs.iter().enumerate() {
                    let selected = self.active_tab == i;
                    let response = ui.selectable_label(selected, &tab.label);

                    if response.clicked() {
                        self.active_tab = i;
                    }

                    if tab.closable {
                        let close_response = ui.small_button("✖");
                        if close_response.clicked() {
                            to_remove = Some(i);
                        }
                    }
                }

                if let Some(index) = to_remove {
                    self.tabs.remove(index);
                    if self.active_tab >= index && self.active_tab > 0 {
                        self.active_tab -= 1;
                    }
                }
            });
        }

        // 保存会话
        fn save_session(&mut self) {
            if self.name.is_empty() || self.group_name.is_empty() {
                self.error_message = Some("名称和分组不能为空".to_string());
                return;
            }
            self.is_loading = true;
            self.error_message = None;

            let db = self.db_manager.clone();
            let name = self.name.clone();
            let group = self.group_name.clone();
            let ip = self.ip.clone();
            let port = self.port.clone();
            let user = self.user_name.clone();
            let pwd = self.password.clone();

            // ② 创建通道
            let (tx, rx) = mpsc::channel();

            // ③ 把任务扔进 Tokio runtime
            let handle = Handle::current().spawn(async move {
                let res = db.add_session(name, group, ip, port, user, pwd).await;
                let _ = tx.send(res); // 把结果送回主线程
            });

            self.rx = Some(rx);
            self.handle = Some(handle);

            println!(
                "✅ 已写入数据库：{} {}:{} {} {} {}",
                self.name, self.group_name, self.ip, self.port, self.user_name, self.password
            );
            self.name.clear();
            self.group_name.clear();
            self.ip.clear();
            self.port.clear();
            self.user_name.clear();
            self.password.clear();
        }


        fn edit_session(&mut self) {
            if self.name.is_empty() || self.group_name.is_empty() {
                self.error_message = Some("名称和分组不能为空".to_string());
                return;
            }
            self.is_loading = true;
            self.error_message = None;

            let db = self.db_manager.clone();
            let name = self.name.clone();
            let group = self.group_name.clone();
            let ip = self.ip.clone();
            let port = self.port.clone();
            let user = self.user_name.clone();
            let pwd = self.password.clone();
            let id=self.id.clone();

            tokio::spawn(async move {
               db.update_session(id, name, group, ip, port, user, pwd).await.expect("更新错误");
            });
            println!(
                "✅ 已更新数据库：{} {}:{} {} {} {}",
                self.name, self.group_name, self.ip, self.port, self.user_name, self.password
            );
            self.name.clear();
            self.group_name.clear();
            self.ip.clear();
            self.port.clear();
            self.user_name.clear();
            self.password.clear();
        }

        fn left_ui(&mut self, ui: &mut egui::Ui) {
            // 宽度
            let desired_width = 40.0;

            let image_size = egui::Vec2::new(24.0, 24.0);

            let home_svg = egui::include_image!("../../data/首页.svg");
            let add_svg = egui::include_image!("../../data/文件添加.svg");
            let set_svg = egui::include_image!("../../data/设置.svg");
            let help_svg = egui::include_image!("../../data/帮助.svg");

            ui.vertical_centered_justified(|ui| {
                ui.set_width(desired_width);
                if style::hover_icon_with_bg(ui, home_svg, image_size).clicked() {
                    self.page = Page::Home;
                    self.active_tab = 0;
                }
                if style::hover_icon_with_bg(ui, add_svg, image_size).clicked() {
                    self.show_add_dialog = true;
                };
                ui.add_space(ui.available_height() - 120.0);
                if style::hover_icon_with_bg(ui, set_svg, image_size).clicked() {
                    self.page = Page::Settings;
                    self.active_tab = 1;
                    self.add_settings_tab();
                };
                if style::hover_icon_with_bg(ui, help_svg, image_size).clicked() {
                    self.show_about_dialog = true;
                };
            });
        }

        fn add_terminal_tab(&mut self, name: String) {
            let already_open = self
                .tabs
                .iter()
                .any(|t| matches!(&t.page, Page::Terminal(n) if n == &name));
            if !already_open {
                self.tabs.push(Tab {
                    label: format!("终端: {}", name),
                    page: Page::Terminal(name.clone()),
                    closable: true,
                });
                self.active_tab = self.tabs.len() - 1;
            }
        }

        fn show_list(&mut self, ui: &mut egui::Ui) {
            let mut to_remove = Vec::new();

            let mut to_name = "";
            TableBuilder::new(ui)
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::remainder()) // 名称列
                .column(Column::remainder()) // 地址列
                .column(Column::remainder()) // 分组列
                .column(Column::remainder()) // 操作列
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.heading("名称");
                    });
                    header.col(|ui| {
                        ui.heading("地址");
                    });
                    header.col(|ui| {
                        ui.heading("分组");
                    });
                    header.col(|ui| {
                        ui.heading("操作");
                    });
                })
                .body(|mut body| {
                    for s in &self.sessions {
                        body.row(30.0, |mut row| {
                            row.col(|ui| {
                                ui.label(&s.name);
                            });
                            row.col(|ui| {
                                ui.label(&format!("{}:{}", s.ip, s.port));
                            });
                            row.col(|ui| {
                                ui.monospace(format!("{}", s.group_name));
                            });
                            row.col(|ui| {
                                if ui.button("连接").clicked() {
                                    // 连接操作
                                    to_name = &*s.name;
                                }
                                if ui.button("编辑").clicked() {
                                    self.show_editor=true;
                                    self.id=s.id;
                                    self.name=s.name.clone();
                                    self.group_name=s.group_name.clone();
                                    self.ip=s.ip.clone();
                                    self.port=s.port.clone();
                                    self.user_name=s.user_name.clone();
                                    self.password=s.password.clone();
                                    // 编辑操作
                                }
                                if ui.button("删除").clicked() {
                                    // 删除操作
                                    to_remove.push(s.id);
                                }
                            });
                        });
                    }
                });

            if !to_name.is_empty() {
                self.add_terminal_tab(to_name.to_string());
            }

            // 在循环外执行删除操作
            for &id in &to_remove {
                // 注意这里的 &to_remove 和 &id
                let db_manager = self.db_manager.clone();
                tokio::spawn(async move {
                    db_manager.delete_session(id).await.unwrap_or_default();
                });

                self.sessions.retain(|session| session.id != id);
            }
            if !to_remove.is_empty() {
                ui.ctx().request_repaint();
            }
        }

        fn show_terminal_ui(&mut self, ui: &mut egui::Ui, name: &str) {
            // 查找对应的会话信息
            let session_info = match self.sessions.iter().find(|s| s.name == name) {
                Some(info) => info.clone(),
                None => {
                    ui.label("会话信息不存在");
                    return;
                }
            };

            if self.terminal_tx.is_none() {
                self.setup_terminal_connection(&session_info);
            }

            ui.vertical(|ui| {
                // 显示终端标题栏
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("🧹 清屏").clicked() {
                            self.terminal_output.clear();
                        }
                    });
                });

                ui.separator();

                // 终端输出区域 - 只显示当前终端内容
                let terminal_frame = egui::Frame::new()
                    .fill(egui::Color32::from_rgb(16, 16, 16))
                    .inner_margin(egui::Margin::symmetric(8.0 as i8, 4.0 as i8));

                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        terminal_frame.show(ui, |ui| {
                            ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                            ui.style_mut().visuals.override_text_color =
                                Some(egui::Color32::from_rgb(220, 220, 220));

                            for line in &self.terminal_output {
                                if line != self.terminal_input2.trim() {
                                    ui.label(line);
                                }
                            }
                            // 当前输入行
                            ui.horizontal(|ui| {
                                // 1. 先画提示符
                                if !self.remote_prompt.is_empty() {
                                    ui.label(
                                        egui::RichText::new(&self.remote_prompt)
                                            .color(egui::Color32::WHITE),
                                    );
                                }

                                // 2. 再画输入框（无边框，无前缀）
                                let input_response = ui.add(
                                    egui::TextEdit::singleline(&mut self.terminal_input)
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(f32::INFINITY)
                                        .frame(false)
                                        .text_color(egui::Color32::WHITE),
                                );

                                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    self.send_terminal_command();
                                }

                                if !input_response.has_focus() {
                                    input_response.request_focus();
                                }
                            });
                        });
                    });

                // 状态栏
                ui.horizontal(|ui| {
                    ui.label("📟");
                    ui.small(format!("行数: {}", self.terminal_output.len()));

                    if let Some(_) = &self.terminal_tx {
                        ui.colored_label(egui::Color32::LIGHT_GREEN, "● 已连接");
                    } else {
                        ui.colored_label(egui::Color32::LIGHT_RED, "● 未连接");
                    }
                });
            });

            // 处理从终端接收的数据
            self.process_terminal_output();
        }

        fn setup_terminal_connection(&mut self, session_info: &sqlite::Session) {
            let host = session_info.ip.clone();
            let port = session_info.port.clone();
            let username = session_info.user_name.clone();
            let password = session_info.password.clone();

            // 创建通道用于与终端线程通信
            let (command_tx, command_rx) = mpsc::channel::<String>();
            let (output_tx, output_rx) = mpsc::channel::<String>();

            // 重置停止标志
            self.terminal_should_stop.store(false, Ordering::Relaxed);

            self.terminal_tx = Some(command_tx);
            self.terminal_rx = Some(output_rx);
            self.terminal_output.clear();
            self.first_banner_arrived = false;

            // 启动终端线程
            let handle = thread::spawn(move || {
                if let Err(e) = Self::run_terminal_session(
                    host, port, username, password, command_rx, output_tx,
                ) {
                    eprintln!("终端会话错误: {}", e);
                }
            });

            self.terminal_thread_handle = Some(handle);
        }

        fn run_terminal_session(
            host: String,
            port: String,
            username: String,
            password: String,
            command_rx: mpsc::Receiver<String>,
            output_tx: mpsc::Sender<String>,
        ) -> Result<(), String> {
            // 建立 TCP 连接
            let tcp = TcpStream::connect(format!("{}:{}", host, port))
                .map_err(|e| format!("TCP连接失败: {}", e))?;

            // 创建 SSH 会话
            let mut sess = Session::new().map_err(|e| format!("创建SSH会话失败: {}", e))?;


            sess.set_tcp_stream(tcp);
            sess.handshake()
                .map_err(|e| format!("SSH握手失败: {}", e))?;

            // 认证
            sess.userauth_password(&username, &password)
                .map_err(|e| format!("认证失败: {}", e))?;

            if !sess.authenticated() {
                return Err("SSH认证失败".to_string());
            }

            // 请求伪终端
            let mut channel = sess
                .channel_session()
                .map_err(|e| format!("创建通道失败: {}", e))?;

            channel
                .request_pty("xterm", None, Some((80, 24, 0, 0)))
                .map_err(|e| format!("请求PTY失败: {}", e))?;

            channel
                .shell()
                .map_err(|e| format!("启动shell失败: {}", e))?;

            // 设置通道为非阻塞模式
            sess.set_blocking(false);

            // 主循环：处理输入输出
            loop {
                // 读取远程输出
                let mut buffer = [0u8; 1024];
                match channel.read(&mut buffer) {
                    Ok(0) => {
                        // 连接关闭
                        let _ = output_tx.send("\r\n连接已关闭".to_string());
                        break;
                    }
                    Ok(n) => {
                        let output = String::from_utf8_lossy(&buffer[..n]).to_string();
                        let _ = output_tx.send(output);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // 没有数据可读，继续
                    }
                    Err(e) => {
                        eprintln!("读取错误: {}", e);
                        break;
                    }
                }

                // 检查是否有命令要发送
                if let Ok(command) = command_rx.try_recv() {
                    if command == "exit" {
                        break;
                    }
                    if channel.write(command.as_bytes()).is_err() {
                        break;
                    }
                    // 发送回车
                    let _ = channel.write(b"\r\n");
                }

                // 短暂休眠以避免CPU占用过高
                thread::sleep(std::time::Duration::from_millis(10));
            }

            // 清理
            let _ = channel.close();
            let _ = channel.wait_close();

            Ok(())
        }

        fn send_terminal_command(&mut self) {
            let command = self.terminal_input.trim();
            if command.is_empty() {
                return;
            }
            self.terminal_output
                .push(format!("{} {}", self.remote_prompt, command));
            // 本地处理的清屏
            if command == "clear" || command == "cls" {
                self.terminal_output.clear();
                self.terminal_input.clear();
                return;
            }

            // 真正把命令发给 SSH 线程
            if let Some(tx) = &self.terminal_tx {
                if tx.send(command.to_string()).is_err() {
                    self.terminal_output
                        .push("❌ 发送命令失败，连接可能已断开".to_string());
                }
            } else {
                self.terminal_output.push("❌ 未连接到终端".to_string());
            }
            self.terminal_input2 = self.terminal_input.clone().trim().to_string();
            self.terminal_input.clear();
        }

        fn strip_control_chars(&mut self, text: &str) -> String {
            let mut result = String::new();
            let mut chars = text.chars().peekable();

            while let Some(c) = chars.next() {
                if c == '\x1B' {
                    // ESC 字符
                    // 处理 ANSI 转义序列
                    if let Some('[') = chars.peek() {
                        chars.next(); // 跳过 '['
                        // 跳过直到字母字符
                        while let Some(&next_char) = chars.peek() {
                            if next_char.is_ascii_alphabetic() {
                                chars.next(); // 跳过结束字符
                                break;
                            }
                            chars.next();
                        }
                    } else if let Some(']') = chars.peek() {
                        // 处理 OSC 命令 (例如 ]0; 和 ]1337;)
                        chars.next(); // 跳过 ']'
                        // 跳过直到 BEL 字符或字符串结束
                        while let Some(&next_char) = chars.peek() {
                            if next_char == '\x07' {
                                // BEL 字符
                                chars.next();
                                break;
                            } else if next_char == '\x1B' && chars.clone().nth(1) == Some('\\') {
                                // 或者 ST 序列 (ESC\)
                                chars.next(); // 跳过 ESC
                                chars.next(); // 跳过 '\'
                                break;
                            }
                            chars.next();
                        }
                    } else {
                        // 其他 ESC 序列，跳过下一个字符
                        if let Some(_) = chars.peek() {
                            chars.next();
                        }
                    }
                } else if c.is_control() && c != '\n' && c != '\r' && c != '\t' {
                    // 跳过其他控制字符，但保留换行、回车、制表符
                    continue;
                } else {
                    result.push(c);
                }
            }
            result
        }

        fn process_terminal_output(&mut self) {
            if let Some(rx) = self.terminal_rx.take() {
                let outputs: Vec<String> = rx.try_iter().collect();
                self.terminal_rx = Some(rx);

                for output in outputs {
                    let cleaned = self.strip_control_chars(&output);
                    let lines = self.process_terminal_data(&cleaned);

                    for line in lines {
                        if !line.trim().is_empty() {
                            self.terminal_output.push(line);
                        }
                    }

                    if self.terminal_output.len() > 500 {
                        self.terminal_output.drain(0..100);
                    }
                }
            }
        }

        fn process_terminal_data(&mut self, data: &str) -> Vec<String> {
            let mut output_lines = Vec::new();

            for line in data.lines() {
                let trimmed = line.trim();
                if trimmed.ends_with('$') && !trimmed.contains(' ') {
                    // ✅ 仅缓存，不打印
                    self.remote_prompt = format!("{} ", trimmed);
                    continue;
                }
                output_lines.push(line.to_string());
            }

            output_lines
        }

        // 展示添加会话弹窗
        fn show_add_dialog(&mut self, ui: &mut egui::Ui) {
            let Self {
                show_add_dialog: _,
                page: _,
                name,
                group_name,
                ip,
                port,
                user_name,
                password,
                select_button,
                select_connect,
                picked_path: _,
                is_loading: _,
                ..
            } = self;

            if let Some(rx) = &mut self.test_rx {
                // 非阻塞收一次
                if let Ok(res) = rx.try_recv() {
                    match res {
                        Ok(msg) => self.test_message = Some(msg),
                        Err(e) => self.test_message = Some(format!("测试失败: {}", e)),
                    }
                    self.test_rx = None; // 收完清掉
                } else {
                    // 还没好，下一帧继续
                    ui.ctx().request_repaint();
                }
            }
            let modal = Modal::new(Id::new("Modal A")).show(ui.ctx(), |ui| {
                ui.set_width(1000.0);

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.heading("名称");
                        ui.add(egui::TextEdit::singleline(name).font(egui::TextStyle::Heading));
                        ui.add_space(20.0);
                        ui.heading("分组");
                        ui.add(
                            egui::TextEdit::singleline(group_name).font(egui::TextStyle::Heading),
                        );
                    });

                    ui.separator();
                    ui.vertical(|ui| {
                        ui.heading("基本");
                        ui.add_space(20.0);

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.heading("连接方式");
                                ui.add_space(10.0);
                                egui::ComboBox::from_label("")
                                    .selected_text(format!("{select_connect:?}"))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            select_connect,
                                            ConnetStyle::直连,
                                            "直连",
                                        );
                                        ui.selectable_value(
                                            select_connect,
                                            ConnetStyle::Http,
                                            "http",
                                        );
                                        ui.selectable_value(
                                            select_connect,
                                            ConnetStyle::Socket,
                                            "socket",
                                        );
                                        ui.selectable_value(
                                            select_connect,
                                            ConnetStyle::Jumphost,
                                            "jumphost",
                                        );
                                    });
                                ui.end_row();
                            });
                            ui.add_space(20.0);
                            ui.vertical(|ui| {
                                ui.heading("主机");
                                ui.add_space(10.0);
                                ui.add(
                                    egui::TextEdit::singleline(ip)
                                        .hint_text("请输入ip地址")
                                        .font(egui::TextStyle::Heading),
                                );
                            });
                            ui.vertical(|ui| {
                                ui.heading("端口");
                                ui.add_space(10.0);
                                ui.add(
                                    egui::TextEdit::singleline(port).font(egui::TextStyle::Heading),
                                );
                            });
                        });
                        ui.add_space(20.0);
                        ui.heading("认证方式");
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.selectable_value(select_button, CertStyle::First, "密码");
                            ui.selectable_value(select_button, CertStyle::Second, "秘钥");
                        });
                        ui.heading("用户名");
                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(user_name)
                                    .font(egui::TextStyle::Heading),
                            );
                            if let Some(error) = &self.error_message {
                                ui.colored_label(egui::Color32::RED, error);
                            }
                            if let Some(text) = &self.test_message {
                                let color = if text.contains("成功") {
                                    egui::Color32::GREEN
                                } else if text.contains("失败") || text.contains("不能为空") {
                                    egui::Color32::RED
                                } else {
                                    egui::Color32::GRAY
                                };
                                ui.colored_label(color, text);
                            }
                        });
                        ui.add_space(20.0);

                        if *select_button == CertStyle::First {
                            ui.heading("密码");
                            ui.add_space(10.0);
                            ui.add(
                                egui::TextEdit::singleline(password)
                                    .hint_text("请输入密码")
                                    .password(true)
                                    .font(egui::TextStyle::Heading),
                            );
                        } else {
                            if ui.button("上传密钥").clicked() {
                                if let Some(path) = rfd::FileDialog::new().pick_file() {
                                    self.picked_path = Some(path.display().to_string());
                                }
                            }
                            if let Some(picked_path) = &self.picked_path {
                                ui.horizontal(|ui| {
                                    ui.label("已选择文件:");
                                    ui.monospace(picked_path);
                                });
                            }
                        }
                    });
                });

                ui.separator();


                egui::Sides::new().show(
                    ui,
                    |_ui| {},
                    |ui| {
                        if ui.button("保存").clicked() {
                            if name.is_empty() {
                                self.error_message = Some("名称不能为空".to_string());
                            } else if group_name.is_empty() {
                                self.error_message = Some("分组不能为空".to_string());
                            } else if ip.is_empty() {
                                self.error_message = Some("IP地址不能为空".to_string());
                            } else if port.is_empty() {
                                self.error_message = Some("端口不能为空".to_string());
                            } else if user_name.is_empty() {
                                self.error_message = Some("用户名不能为空".to_string());
                            } else if *select_button == CertStyle::First && password.is_empty() {
                                self.error_message = Some("密码不能为空".to_string());
                            } else {
                                self.error_message = None;
                                self.should_save = true;
                                ui.close();
                            }
                        }
                        if ui.button("测试连接").clicked() {
                            self.cmd = Cmd::Test {
                                host: ip.clone(),
                                port: port.clone(),
                                user: user_name.clone(),
                                pwd:  password.clone(),
                                style: select_button.clone(),
                            };
                        }
                        if ui.button("退出").clicked() {
                            let sessions1 = task::block_in_place(|| {
                                Handle::current().block_on(self.db_manager.get_sessions())
                            })
                            .unwrap_or_default();
                            // 打印 sessions1 的内容
                            println!("退出时的 sessions1: {:?}", sessions1);
                            ui.close();
                        }
                    },
                );

            });
            if modal.should_close() {
                self.show_add_dialog = false;
            }
        }


        // 展示编辑会话弹窗
        fn show_edit_dialog(&mut self, ui: &mut egui::Ui) {
            let Self {
                show_add_dialog: _,
                page: _,
                name,
                group_name,
                ip,
                port,
                user_name,
                password,
                select_button,
                select_connect,
                picked_path: _,
                is_loading: _,
                ..
            } = self;

            if let Some(rx) = &mut self.test_rx {
                // 非阻塞收一次
                if let Ok(res) = rx.try_recv() {
                    match res {
                        Ok(msg) => self.test_message = Some(msg),
                        Err(e) => self.test_message = Some(format!("测试失败: {}", e)),
                    }
                    self.test_rx = None; // 收完清掉
                } else {
                    // 还没好，下一帧继续
                    ui.ctx().request_repaint();
                }
            }
            let modal = Modal::new(Id::new("Modal A")).show(ui.ctx(), |ui| {
                ui.set_width(1000.0);

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.heading("名称");
                        ui.add(egui::TextEdit::singleline(name).font(egui::TextStyle::Heading));
                        ui.add_space(20.0);
                        ui.heading("分组");
                        ui.add(
                            egui::TextEdit::singleline(group_name).font(egui::TextStyle::Heading),
                        );
                    });

                    ui.separator();
                    ui.vertical(|ui| {
                        ui.heading("基本");
                        ui.add_space(20.0);

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.heading("连接方式");
                                ui.add_space(10.0);
                                egui::ComboBox::from_label("")
                                    .selected_text(format!("{select_connect:?}"))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            select_connect,
                                            ConnetStyle::直连,
                                            "直连",
                                        );
                                        ui.selectable_value(
                                            select_connect,
                                            ConnetStyle::Http,
                                            "http",
                                        );
                                        ui.selectable_value(
                                            select_connect,
                                            ConnetStyle::Socket,
                                            "socket",
                                        );
                                        ui.selectable_value(
                                            select_connect,
                                            ConnetStyle::Jumphost,
                                            "jumphost",
                                        );
                                    });
                                ui.end_row();
                            });
                            ui.add_space(20.0);
                            ui.vertical(|ui| {
                                ui.heading("主机");
                                ui.add_space(10.0);
                                ui.add(
                                    egui::TextEdit::singleline(ip)
                                        .hint_text("请输入ip地址")
                                        .font(egui::TextStyle::Heading),
                                );
                            });
                            ui.vertical(|ui| {
                                ui.heading("端口");
                                ui.add_space(10.0);
                                ui.add(
                                    egui::TextEdit::singleline(port).font(egui::TextStyle::Heading),
                                );
                            });
                        });
                        ui.add_space(20.0);
                        ui.heading("认证方式");
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.selectable_value(select_button, CertStyle::First, "密码");
                            ui.selectable_value(select_button, CertStyle::Second, "秘钥");
                        });
                        ui.heading("用户名");
                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(user_name)
                                    .font(egui::TextStyle::Heading),
                            );
                            if let Some(error) = &self.error_message {
                                ui.colored_label(egui::Color32::RED, error);
                            }
                            if let Some(text) = &self.test_message {
                                let color = if text.contains("成功") {
                                    egui::Color32::GREEN
                                } else if text.contains("失败") || text.contains("不能为空") {
                                    egui::Color32::RED
                                } else {
                                    egui::Color32::GRAY
                                };
                                ui.colored_label(color, text);
                            }
                        });
                        ui.add_space(20.0);

                        if *select_button == CertStyle::First {
                            ui.heading("密码");
                            ui.add_space(10.0);
                            ui.add(
                                egui::TextEdit::singleline(password)
                                    .hint_text("请输入密码")
                                    .password(true)
                                    .font(egui::TextStyle::Heading),
                            );
                        } else {
                            if ui.button("上传密钥").clicked() {
                                if let Some(path) = rfd::FileDialog::new().pick_file() {
                                    self.picked_path = Some(path.display().to_string());
                                }
                            }
                            if let Some(picked_path) = &self.picked_path {
                                ui.horizontal(|ui| {
                                    ui.label("已选择文件:");
                                    ui.monospace(picked_path);
                                });
                            }
                        }
                    });
                });

                ui.separator();


                egui::Sides::new().show(
                    ui,
                    |_ui| {},
                    |ui| {
                        if ui.button("保存").clicked() {
                            if name.is_empty() {
                                self.error_message = Some("名称不能为空".to_string());
                            } else if group_name.is_empty() {
                                self.error_message = Some("分组不能为空".to_string());
                            } else if ip.is_empty() {
                                self.error_message = Some("IP地址不能为空".to_string());
                            } else if port.is_empty() {
                                self.error_message = Some("端口不能为空".to_string());
                            } else if user_name.is_empty() {
                                self.error_message = Some("用户名不能为空".to_string());
                            } else if *select_button == CertStyle::First && password.is_empty() {
                                self.error_message = Some("密码不能为空".to_string());
                            } else {
                                self.error_message = None;
                                self.should_update = true;
                                ui.close();
                            }
                        }
                        if ui.button("测试连接").clicked() {
                            self.cmd = Cmd::Test {
                                host: ip.clone(),
                                port: port.clone(),
                                user: user_name.clone(),
                                pwd:  password.clone(),
                                style: select_button.clone(),
                            };
                        }
                        if ui.button("退出").clicked() {
                            let sessions1 = task::block_in_place(|| {
                                Handle::current().block_on(self.db_manager.get_sessions())
                            })
                                .unwrap_or_default();
                            // 打印 sessions1 的内容
                            println!("退出时的 sessions1: {:?}", sessions1);
                            self.show_editor=false;
                            ui.close();
                        }
                    },
                );

            });
            if modal.should_close() {
                self.show_add_dialog = false;
            }
        }


        // 测试连接
        fn test_connection(
            &mut self,
            ui: &mut egui::Ui,
            ip: &mut String,
            port: &mut String,
            user_name: &mut String,
            password: &mut String,
            select_button: &mut CertStyle,
        ) {
            // 1. 先简单校验
            if ip.is_empty() || port.is_empty() || user_name.is_empty() {
                self.test_message = Some("请填写主机、端口和用户名".into());
            } else if *select_button == CertStyle::First && password.is_empty() {
                self.test_message = Some("密码不能为空".into());
            } else if *select_button == CertStyle::Second && self.picked_path.is_none() {
                self.test_message = Some("请选择密钥文件".into());
            } else {
                // 2. 把需要的数据克隆出来，准备 move 到线程里
                let host = ip.clone();
                let port = port.clone();
                let user = user_name.clone();
                let pwd = password.clone();
                let key_path = self.picked_path.clone();
                let use_pwd = *select_button == CertStyle::First;

                // 3. 创建 channel，主线程收，子线程发
                let (tx, rx) = mpsc::channel::<Result<String, String>>();

                // 4. 在 tokio 线程池里做阻塞的 ssh 连接
                task::spawn_blocking(move || {
                    let res = (|| -> Result<String, String> {
                        // 4.1 TCP 连接
                        let tcp = TcpStream::connect(format!("{}:{}", host, port))
                            .map_err(|e| format!("TCP 连接失败: {}", e))?;
                        // 4.2 SSH 握手
                        let mut sess =
                            Session::new().map_err(|e| format!("创建 SSH Session 失败: {}", e))?;
                        sess.set_tcp_stream(tcp);
                        sess.handshake()
                            .map_err(|e| format!("SSH 握手失败: {}", e))?;
                        // 4.3 认证
                        if use_pwd {
                            sess.userauth_password(&user, &pwd)
                                .map_err(|e| format!("密码认证失败: {}", e))?;
                        } else {
                            // 密钥测试
                            let key_path = key_path.ok_or("未选择密钥文件")?;
                            let key_path = Path::new(&key_path);  // 转换为 &Path
                            sess.userauth_pubkey_file(&user, None, key_path, None)
                                .map_err(|e| format!("密钥认证失败: {}", e))?;
                        }
                        // 4.4 简单执行一条命令验证通道
                        let mut channel = sess
                            .channel_session()
                            .map_err(|e| format!("打开通道失败: {}", e))?;
                        channel
                            .exec("echo OK")
                            .map_err(|e| format!("执行命令失败: {}", e))?;
                        let mut out = String::new();
                        channel
                            .read_to_string(&mut out)
                            .map_err(|e| format!("读取回显失败: {}", e))?;
                        channel
                            .wait_close()
                            .map_err(|e| format!("关闭通道失败: {}", e))?;
                        Ok(format!("连接成功！远程回显: {}", out.trim()))
                    })();
                    // 把结果发回主线程，忽略 send 错误（主线程可能已关闭）
                    let _ = tx.send(res);
                });

                // 5. 主线程把 channel 保存到 self，在下一帧循环里收结果
                self.test_rx = Some(rx);
                self.test_message = Some("正在测试连接…".into());

                if let Some(rx) = &mut self.test_rx {
                    if let Ok(res) = rx.try_recv() {
                        match res {
                            Ok(msg) => self.test_message = Some(msg),
                            Err(e) => self.test_message = Some(format!("测试失败: {}", e)),
                        }
                        self.test_rx = None; // 收完就清掉
                    } else {
                        // 还没收到，告诉 egui 下一帧继续刷
                        ui.ctx().request_repaint();
                    }
                }
            }
        }

        fn add_settings_tab(&mut self) {
            let already_open = self.tabs.iter().any(|t| t.page == Page::Settings);
            if !already_open {
                self.tabs.push(Tab {
                    label: "设置".to_string(),
                    page: Page::Settings,
                    closable: true,
                });
                self.active_tab = self.tabs.len() - 1;
            }
        }

        fn show_page(&mut self, ui: &mut egui::Ui) {
            if let Some(tab) = self.tabs.get(self.active_tab) {
                match &tab.page {
                    Page::Home => {
                        ui.heading("首页会话列表");
                        ui.separator();
                        self.show_list(ui)
                    }
                    Page::Settings => {
                        ui.heading("设置页面");
                        ui.label("这里是设置内容");
                    }
                    Page::Terminal(name) => {
                        let name = name.clone(); // ✅ 先克
                        self.show_terminal_ui(ui, &name);
                    }
                }
            }
        }

        fn main_ui(&mut self, ui: &mut egui::Ui) {
            egui::SidePanel::left("left_panel")
                .resizable(true)
                .default_width(150.0)
                .width_range(80.0..=200.0)
                .show_inside(ui, |ui| {
                    self.left_ui(ui);
                });

            CentralPanel::default().show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_tab_bar(ui);
                    self.show_page(ui);
                });
            });
        }
    }

    impl eframe::App for MyEguiApp {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            CentralPanel::default().show(ctx, |ui| {
                self.main_ui(ui);

                if let Cmd::Test { host, port, user, pwd, style } = std::mem::take(&mut self.cmd) {
                    let mut host = host;
                    let mut port = port;
                    let mut user = user;
                    let mut pwd  = pwd;
                    let mut style = style;
                    // 临时 UI 只用一次，用完即扔
                    let mut ui = egui::Ui::new(
                        ui.ctx().clone(),
                        Id::new("test_ui"),   // 补上 Id
                        egui::UiBuilder::default(),
                    );
                    self.test_connection(&mut ui, &mut host, &mut port, &mut user, &mut pwd, &mut style);
                }

                if ctx.input(|i| i.viewport().close_requested()) {
                    println!("收到关闭请求，清理资源...");
                    return;
                }
                if self.show_add_dialog {
                    self.show_add_dialog(ui);
                }
                if self.show_editor{
                    self.show_edit_dialog(ui);
                }

                if self.show_about_dialog {
                    about::show_about(ui, &mut self.show_about_dialog);
                }
                if self.should_save {
                    self.save_session();
                    let sessions1 = task::block_in_place(|| {
                        Handle::current().block_on(self.db_manager.get_sessions())
                    })
                    .unwrap_or_default();
                    self.sessions = sessions1.clone();
                    self.should_save = false;
                    egui::Context::request_repaint(ctx);
                }

                if self.should_update {
                    self.edit_session();
                    let sessions1 = task::block_in_place(|| {
                        Handle::current().block_on(self.db_manager.get_sessions())
                    })
                        .unwrap_or_default();
                    self.sessions = sessions1.clone();
                    self.should_update = false;
                    self.show_editor=false;
                    egui::Context::request_repaint(ctx);
                }
            });
        }
    }
}
