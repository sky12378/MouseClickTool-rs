//! 双击运行时不弹黑色 cmd 窗口：链接为 Windows GUI 子系统（默认是 console）
#![windows_subsystem = "windows"]

//! MouseClickTool — egui 版（纯 Rust，零手写 FFI，轻量化）
//! 参考鼠大侠界面风格：品牌头部 + 配置表单 + 底部大按钮。
//! 功能：左/右键·长按·滚轮 / 自定义间隔 / 全局热键 / 定时触发 / 点击次数 /
//!       启动外部程序 / 自定义脚本(.msck) / 随机扰动 / 记录日志 / 中英双语 / 配置持久化。

mod config;
mod script;

use config::Config;
use eframe::egui;
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Mouse, Settings};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 动作索引 → (中文名, 英文名)
const ACTIONS: [(&str, &str); 8] = [
    ("左键点击", "Left Click"),
    ("右键点击", "Right Click"),
    ("左键长按", "Left Long Press"),
    ("右键长按", "Right Long Press"),
    ("向上滚动", "Scroll Up"),
    ("向下滚动", "Scroll Down"),
    ("启动程序", "Launch Program"),
    ("自定义脚本", "Custom Script"),
];

const HOTKEYS: [&str; 14] = [
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "Home", "End",
];

/// 中英文案助手
fn t<'a>(cn: bool, zh: &'a str, en: &'a str) -> &'a str {
    if cn {
        zh
    } else {
        en
    }
}

/// 共享运行时状态（worker 线程 → UI）
struct Shared {
    running: bool,
    remaining: u64,
    logs: Vec<String>,
}

/// 全局热键名 → Code
fn hotkey_code(name: &str) -> Code {
    match name {
        "F2" => Code::F2,
        "F3" => Code::F3,
        "F4" => Code::F4,
        "F5" => Code::F5,
        "F6" => Code::F6,
        "F7" => Code::F7,
        "F8" => Code::F8,
        "F9" => Code::F9,
        "F10" => Code::F10,
        "F11" => Code::F11,
        "F12" => Code::F12,
        "Home" => Code::Home,
        "End" => Code::End,
        _ => Code::F1,
    }
}

/// 当日秒数（UTC 无关，本地当日 0 点起的秒，用于定时触发比较）
fn now_seconds_of_day() -> u64 {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    // 北京时间 GMT+8 固定偏移（HPC 场景主机多为东八区）
    (d.as_secs() + 8 * 3600) % 86400
}

/// 可中断 sleep：返回 false 表示被取消
fn interruptible_sleep(ms: u64, cancel: &AtomicBool) -> bool {
    let step = 50u64;
    let mut left = ms;
    while left > 0 {
        if cancel.load(Ordering::SeqCst) {
            return false;
        }
        let s = left.min(step);
        std::thread::sleep(Duration::from_millis(s));
        left -= s;
    }
    !cancel.load(Ordering::SeqCst)
}

/// 连点工作线程（enigo 鼠标模拟）
fn worker(cfg: Config, cancel: Arc<AtomicBool>, shared: Arc<Mutex<Shared>>, ctx: egui::Context) {
    let mut log = |s: String| {
        let mut sh = shared.lock().unwrap();
        sh.logs.push(s);
        if sh.logs.len() > 200 {
            sh.logs.remove(0);
        }
    };

    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(e) => e,
        Err(e) => {
            log(format!("enigo init failed: {:?}", e));
            return;
        }
    };

    // 定时触发：等待到目标时间（h/m/s 防配置超界，clamp 到合法范围）
    let target_secs = cfg.trig_h.min(23) as u64 * 3600
        + cfg.trig_m.min(59) as u64 * 60
        + cfg.trig_s.min(59) as u64;
    if target_secs > 0 {
        let now = now_seconds_of_day();
        let wait = if target_secs > now {
            target_secs - now
        } else {
            86400 - now + target_secs
        };
        log(format!("等待定时触发 {}s", wait));
        if !interruptible_sleep(wait * 1000, &cancel) {
            return;
        }
    }

    // 启动程序模式：执行一次
    if cfg.action == 6 {
        let _ = std::process::Command::new(&cfg.path).spawn();
        log("已启动程序".to_string());
        let mut sh = shared.lock().unwrap();
        sh.running = false;
        return;
    }

    // 脚本模式：加载脚本
    let script_cmds = if cfg.action == 7 {
        match std::fs::read_to_string(cfg.path.trim()) {
            Ok(content) => Some(script::parse_script(&content)),
            Err(_) => {
                log("脚本读取失败".to_string());
                let mut sh = shared.lock().unwrap();
                sh.running = false;
                return;
            }
        }
    } else {
        None
    };
    let script_len = script_cmds.as_ref().map(|v| v.len()).unwrap_or(0);
    let mut script_index = 0usize;

    let unrestricted = cfg.count == 0;
    let mut count: u64 = 0;
    let mut pressed = false; // 长按已按下标志
    let mut script_hold_l = false; // 脚本 left_click_long 按下状态（防卡键）
    let mut script_hold_r = false; // 脚本 right_click_long 按下状态（防卡键）
    let right = cfg.action == 1 || cfg.action == 3;
    let long_press = cfg.action == 2 || cfg.action == 3;
    let mouse_wheel = cfg.action == 4 || cfg.action == 5;

    loop {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        if !unrestricted && count >= cfg.count {
            break;
        }

        if script_len > 0 {
            // 脚本模式：每循环执行一条，回绕
            if script_index >= script_len {
                script_index = 0;
            }
            let cmd = &script_cmds.as_ref().unwrap()[script_index];
            script_index += 1;
            match cmd {
                script::ScriptCmd::Delay(ms) => {
                    if !interruptible_sleep(*ms, &cancel) {
                        break;
                    }
                }
                script::ScriptCmd::Title(title) => {
                    let mut sh = shared.lock().unwrap();
                    sh.logs.push(format!("title -> {}", title));
                }
                script::ScriptCmd::LeftClick(pos) => {
                    click_pos(&mut enigo, pos, &cfg, Button::Left);
                    if cfg.record {
                        log(format!("left_click {:?}", pos));
                    }
                }
                script::ScriptCmd::RightClick(pos) => {
                    click_pos(&mut enigo, pos, &cfg, Button::Right);
                    if cfg.record {
                        log(format!("right_click {:?}", pos));
                    }
                }
                script::ScriptCmd::LeftLong(pos, press) => {
                    long_pos(&mut enigo, pos, &cfg, Button::Left, *press);
                    script_hold_l = *press;
                }
                script::ScriptCmd::RightLong(pos, press) => {
                    long_pos(&mut enigo, pos, &cfg, Button::Right, *press);
                    script_hold_r = *press;
                }
                script::ScriptCmd::Wheel(v) => {
                    let _ = enigo.scroll(*v, Axis::Vertical);
                }
                script::ScriptCmd::Process(p) => {
                    let _ = std::process::Command::new(p).spawn();
                }
                script::ScriptCmd::Once => {
                    break;
                }
                script::ScriptCmd::Exit => {
                    cancel.store(true, Ordering::SeqCst);
                }
            }
        } else {
            // 普通点击模式
            if mouse_wheel {
                let delta = if cfg.action == 4 {
                    cfg.scroll
                } else {
                    -cfg.scroll
                };
                let _ = enigo.scroll(delta, Axis::Vertical);
            } else if long_press {
                if !pressed {
                    let b = if right { Button::Right } else { Button::Left };
                    let _ = enigo.button(b, Direction::Press);
                    pressed = true;
                }
            } else {
                let b = if right { Button::Right } else { Button::Left };
                let _ = enigo.button(b, Direction::Press);
                let _ = enigo.button(b, Direction::Release);
                if cfg.record {
                    log(format!("{}_click", if right { "right" } else { "left" }));
                }
            }
        }

        count += 1;
        if !unrestricted {
            let mut sh = shared.lock().unwrap();
            sh.remaining = cfg.count.saturating_sub(count);
        }

        // 随机扰动：0.8 ~ 1.2 倍间隔
        let interval = if cfg.random {
            (cfg.interval as f64 * (0.8 + rand::random::<f64>() * 0.4)) as u64
        } else {
            cfg.interval
        };
        if !interruptible_sleep(interval, &cancel) {
            break;
        }
        ctx.request_repaint();
    }

    // 长按释放（普通长按模式）
    if long_press && pressed {
        let b = if right { Button::Right } else { Button::Left };
        let _ = enigo.button(b, Direction::Release);
    }
    // 脚本长按残留释放（防止脚本 left_click_long/right_click_long 按下后
    // 因 exit()/停止/异常退出而卡键）
    if script_hold_l {
        let _ = enigo.button(Button::Left, Direction::Release);
    }
    if script_hold_r {
        let _ = enigo.button(Button::Right, Direction::Release);
    }

    let mut sh = shared.lock().unwrap();
    sh.running = false;
    sh.remaining = cfg.count;
    sh.logs.push(t(cfg.cn, "已停止", "Stopped").to_string());
    drop(sh);
    ctx.request_repaint();
}

/// 脚本点击：可选坐标（None 用当前光标）
fn click_pos(enigo: &mut Enigo, pos: &Option<(i32, i32)>, cfg: &Config, button: Button) {
    if let Some((x, y)) = pos {
        let _ = enigo.move_mouse(*x, *y, Coordinate::Abs);
    }
    let _ = enigo.button(button, Direction::Press);
    let _ = enigo.button(button, Direction::Release);
}

fn long_pos(enigo: &mut Enigo, pos: &Option<(i32, i32)>, cfg: &Config, button: Button, press: bool) {
    if let Some((x, y)) = pos {
        let _ = enigo.move_mouse(*x, *y, Coordinate::Abs);
    }
    let d = if press { Direction::Press } else { Direction::Release };
    let _ = enigo.button(button, d);
}

/// 加载系统中文字体（微软雅黑等），解决 egui 默认字体无 CJK 字形导致的"口"字问题。
/// 顺序尝试：微软雅黑 ttc → 黑体 ttf → 等线 ttf → 宋体 ttc。
fn install_cjk_font(ctx: &egui::Context) {
    let candidates = [
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\Deng.ttf",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ];
    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("cjk".to_owned(), egui::FontData::from_owned(data).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                if let Some(list) = fonts.families.get_mut(&family) {
                    list.push("cjk".to_owned());
                }
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
}

/// egui App
struct App {
    cfg: Config,
    shared: Arc<Mutex<Shared>>,
    cancel: Arc<AtomicBool>,
    hotkey_manager: Option<GlobalHotKeyManager>,
    hotkey: HotKey,
    ctx: egui::Context,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_cjk_font(&cc.egui_ctx);
        let cfg = Config::load();
        let ctx = cc.egui_ctx.clone();
        let shared = Arc::new(Mutex::new(Shared {
            running: false,
            remaining: 0,
            logs: vec![t(true, "就绪，点击开始", "Ready. Press Start").to_string()],
        }));
        let hotkey = HotKey::new(None, hotkey_code(&cfg.hotkey));
        let manager = GlobalHotKeyManager::new().ok();
        if let Some(m) = &manager {
            if let Err(e) = m.register(hotkey.clone()) {
                shared.lock().unwrap().logs.push(format!(
                    "⚠️ 热键 {} 注册失败: {:?}（可能被占用）",
                    cfg.hotkey, e
                ));
            }
        }
        App {
            cfg,
            shared,
            cancel: Arc::new(AtomicBool::new(false)),
            hotkey_manager: manager,
            hotkey,
            ctx,
        }
    }

    fn re_register_hotkey(&mut self) {
        if let Some(m) = &self.hotkey_manager {
            let _ = m.unregister(self.hotkey.clone());
            let hk = HotKey::new(None, hotkey_code(&self.cfg.hotkey));
            if let Err(e) = m.register(hk.clone()) {
                self.shared
                    .lock()
                    .unwrap()
                    .logs
                    .push(format!("⚠️ 热键 {} 注册失败: {:?}", self.cfg.hotkey, e));
            }
            self.hotkey = hk;
        }
    }

    fn start(&mut self) {
        let _ = self.cfg.save();
        self.cancel.store(false, Ordering::SeqCst);
        {
            let mut sh = self.shared.lock().unwrap();
            sh.running = true;
            sh.remaining = self.cfg.count;
            sh.logs.clear();
            sh.logs.push(t(self.cfg.cn, "运行中...", "Running...").to_string());
        }
        let cancel = self.cancel.clone();
        let cfg = self.cfg.clone();
        let shared = self.shared.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || worker(cfg, cancel, shared, ctx));
    }

    fn stop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    fn poll_hotkey(&mut self) {
        while let Ok(ev) = GlobalHotKeyEvent::receiver().try_recv() {
            if ev.state() == HotKeyState::Pressed {
                let running = self.shared.lock().unwrap().running;
                if running {
                    self.stop();
                } else {
                    self.start();
                }
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_hotkey();

        let cn = self.cfg.cn;
        let running = self.shared.lock().unwrap().running;
        let remaining = self.shared.lock().unwrap().remaining;
        let logs = self.shared.lock().unwrap().logs.clone();

        // ===== 顶部品牌区 =====
        egui::Panel::top("header").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("MouseClickTool")
                        .size(17.0)
                        .strong()
                        .color(egui::Color32::from_rgb(72, 118, 255)),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(t(cn, "鼠标连点器", "Auto Clicker"))
                        .size(11.0)
                        .color(egui::Color32::GRAY),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (dot, text) = if running {
                        (
                            egui::Color32::from_rgb(255, 82, 82),
                            t(cn, "运行中", "Running"),
                        )
                    } else {
                        (
                            egui::Color32::from_rgb(82, 196, 26),
                            t(cn, "就绪", "Ready"),
                        )
                    };
                    ui.label(egui::RichText::new("●").color(dot).size(12.0));
                    ui.label(egui::RichText::new(text).size(11.0));
                });
            });
            ui.add_space(4.0);
            ui.separator();
        });

        // ===== 底部控制区 =====
        egui::Panel::bottom("control").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                let btn_text = if running {
                    t(cn, "停止 (热键切换)", "Stop (Hotkey toggles)")
                } else {
                    t(cn, "开始", "Start")
                };
                let (bg, fg) = if running {
                    (egui::Color32::from_rgb(255, 82, 82), egui::Color32::WHITE)
                } else {
                    (egui::Color32::from_rgb(72, 118, 255), egui::Color32::WHITE)
                };
                let btn = egui::Button::new(
                    egui::RichText::new(btn_text).size(14.0).strong().color(fg),
                )
                .fill(bg)
                .min_size(egui::vec2(270.0, 34.0))
                .corner_radius(8.0);
                if ui.add(btn).clicked() {
                    if running {
                        self.stop();
                    } else {
                        self.start();
                    }
                }
                ui.add_space(8.0);
                if ui
                    .add(egui::Button::new(t(cn, "英文", "中文")))
                    .on_hover_text(t(cn, "切换语言", "Switch language"))
                    .clicked()
                {
                    self.cfg.cn = !self.cfg.cn;
                    let _ = self.cfg.save();
                }
            });
            // 状态行
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                let remain_text = if self.cfg.count == 0 {
                    t(cn, "无限次数", "Unlimited").to_string()
                } else {
                    format!("{}: {}", t(cn, "剩余次数", "Remaining"), remaining)
                };
                ui.label(egui::RichText::new(remain_text).size(11.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(format!("{}: {}", t(cn, "热键", "Hotkey"), self.cfg.hotkey))
                            .size(11.0)
                            .color(egui::Color32::GRAY),
                    );
                });
            });
            ui.add_space(6.0);
        });

        // ===== 中央配置区 =====
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(2.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add_space(4.0);

                    // 动作类型
                    ui.horizontal(|ui| {
                        ui.label(t(cn, "动作类型", "Action"));
                        ui.add_space(6.0);
                        let current = ACTIONS[self.cfg.action.min(7)];
                        egui::ComboBox::from_id_salt("action")
                            .selected_text(format!("{} / {}", current.0, current.1))
                            .width(180.0)
                            .show_ui(ui, |ui| {
                                for (i, (zh, en)) in ACTIONS.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut self.cfg.action,
                                        i,
                                        format!("{} / {}", zh, en),
                                    );
                                }
                            });
                    });

                    // 间隔 + 随机
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(t(cn, "点击间隔", "Interval"));
                        ui.add_space(6.0);
                        ui.add(
                            egui::DragValue::new(&mut self.cfg.interval)
                                .range(1..=600000)
                                .suffix(" ms")
                                .speed(10.0),
                        );
                        ui.add_space(12.0);
                        ui.checkbox(&mut self.cfg.random, t(cn, "随机扰动 (±20%)", "Random (±20%)"));
                    });

                    // 点击次数
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(t(cn, "点击次数", "Click Count"));
                        ui.add_space(6.0);
                        ui.add(
                            egui::DragValue::new(&mut self.cfg.count)
                                .range(0..=10_000_000)
                                .speed(1.0),
                        );
                        ui.label(
                            egui::RichText::new(t(cn, "(0 = 无限)", "(0 = unlimited)"))
                                .size(11.0)
                                .color(egui::Color32::GRAY),
                        );
                    });

                    // 全局热键
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(t(cn, "全局热键", "Global Hotkey"));
                        ui.add_space(6.0);
                        let before = self.cfg.hotkey.clone();
                        egui::ComboBox::from_id_salt("hotkey")
                            .selected_text(self.cfg.hotkey.clone())
                            .width(100.0)
                            .show_ui(ui, |ui| {
                                for h in HOTKEYS {
                                    ui.selectable_value(&mut self.cfg.hotkey, h.to_string(), h);
                                }
                            });
                        if self.cfg.hotkey != before {
                            self.re_register_hotkey();
                            let _ = self.cfg.save();
                        }
                        ui.label(
                            egui::RichText::new(t(cn, "（点击开始/停止切换）", "(toggles start/stop)"))
                                .size(11.0)
                                .color(egui::Color32::GRAY),
                        );
                    });

                    // 定时触发
                    ui.add_space(4.0);
                    let mut enable_trigger =
                        self.cfg.trig_h > 0 || self.cfg.trig_m > 0 || self.cfg.trig_s > 0;
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut enable_trigger, t(cn, "定时触发", "Timed Trigger"));
                        if enable_trigger {
                            ui.add_space(4.0);
                            ui.add(
                                egui::DragValue::new(&mut self.cfg.trig_h)
                                    .range(0..=23)
                                    .prefix(" ")
                                    .suffix(" h"),
                            );
                            ui.add(
                                egui::DragValue::new(&mut self.cfg.trig_m)
                                    .range(0..=59)
                                    .prefix(" ")
                                    .suffix(" m"),
                            );
                            ui.add(
                                egui::DragValue::new(&mut self.cfg.trig_s)
                                    .range(0..=59)
                                    .prefix(" ")
                                    .suffix(" s"),
                            );
                        }
                    });
                    if !enable_trigger {
                        self.cfg.trig_h = 0;
                        self.cfg.trig_m = 0;
                        self.cfg.trig_s = 0;
                    }

                    // 程序/脚本路径
                    if self.cfg.action == 6 || self.cfg.action == 7 {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(if self.cfg.action == 6 {
                                t(cn, "程序路径", "Program Path")
                            } else {
                                t(cn, "脚本文件", "Script File")
                            });
                            ui.add_space(6.0);
                            let mut path = self.cfg.path.clone();
                            ui.add(
                                egui::TextEdit::singleline(&mut path)
                                    .desired_width(160.0)
                                    .hint_text(t(cn, "输入路径或脚本 (.msck)", "Path or .msck script")),
                            );
                            if ui
                                .add(egui::Button::new(t(cn, "保存", "Save")))
                                .clicked()
                            {
                                self.cfg.path = path;
                                let _ = self.cfg.save();
                            }
                        });
                    }

                    // 记录日志
                    ui.add_space(4.0);
                    ui.checkbox(&mut self.cfg.record, t(cn, "记录日志", "Record Logs"));

                    ui.add_space(4.0);
                });

                // ===== 日志区 =====
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(t(cn, "运行日志", "Log"))
                        .size(11.0)
                        .strong(),
                );
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::vertical()
                        .max_height(90.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for l in logs.iter().rev().take(100) {
                                ui.label(
                                    egui::RichText::new(l)
                                        .size(11.0)
                                        .color(egui::Color32::GRAY),
                                );
                            }
                        });
                });
                ui.add_space(4.0);
            });
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([340.0, 440.0])
            .with_min_inner_size([320.0, 400.0])
            .with_title("MouseClickTool"),
        ..Default::default()
    };
    eframe::run_native(
        "MouseClickTool",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
