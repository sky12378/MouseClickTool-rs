//! 配置持久化模块（egui 版）。
//! 使用 serde_json 持久化到 exe 所在目录的 mouse_click_tool.json（轻量化，零 FFI）。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 应用配置（与 C# 原版功能一一对应）
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    /// 点击间隔（毫秒）
    pub interval: u64,
    /// 动作索引：0左键 1右键 2左长按 3右长按 4滚上 5滚下 6启动程序 7脚本
    pub action: usize,
    /// 滚轮滚动量
    pub scroll: i32,
    /// 全局热键名：F1..F12 / Home / End
    pub hotkey: String,
    /// 随机扰动
    pub random: bool,
    /// 记录日志
    pub record: bool,
    /// 点击次数（空/0 = 无限）
    pub count: u64,
    /// 程序路径 / 脚本路径（按动作模式语义）
    pub path: String,
    /// 定时触发（时/分/秒，00:00:00 表示不启用）
    pub trig_h: u32,
    pub trig_m: u32,
    pub trig_s: u32,
    /// 中英双语
    pub cn: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            interval: 1000,
            action: 0,
            scroll: 600,
            hotkey: "F1".to_string(),
            random: false,
            record: false,
            count: 0,
            path: String::new(),
            trig_h: 0,
            trig_m: 0,
            trig_s: 0,
            cn: true,
        }
    }
}

impl Config {
    /// 配置文件路径（exe 所在目录）
    pub fn path() -> PathBuf {
        PathBuf::from("mouse_click_tool.json")
    }

    /// 加载配置（失败则返回默认）
    pub fn load() -> Self {
        let p = Self::path();
        if let Ok(content) = fs::read_to_string(&p) {
            if let Ok(cfg) = serde_json::from_str(&content) {
                return cfg;
            }
        }
        Config::default()
    }

    /// 保存配置
    pub fn save(&self) -> bool {
        match serde_json::to_string_pretty(self) {
            Ok(s) => fs::write(Self::path(), s).is_ok(),
            Err(_) => false,
        }
    }
}
