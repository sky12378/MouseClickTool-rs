# MouseClickTool — Auto Clicker (Rust / egui)

A lightweight auto clicker written in pure Rust, built on **egui / eframe** with zero handwritten FFI. Ready to use out of the box.

> UI redesigned following the style of "ShuDaXia" (鼠大侠), with feature parity to classic auto clickers: single click, hold, scroll, timed trigger, global hotkeys, and a script engine.

## ✨ Features

- 🖱️ **Multiple actions**: left click / right click / left hold / right hold / middle click / mouse wheel / launch a program / run a script
- ⏱️ **Custom interval**: millisecond precision, with ±20% random jitter to simulate human behavior
- 🔢 **Click count**: finite count or unlimited
- ⌨️ **Global hotkeys**: F1–F12 / Home / End to toggle start/stop (works in the background)
- ⏰ **Timed trigger**: auto-start at a specified h/m/s time
- 📜 **Script engine**: `.msck` script files supporting delays, coordinate clicks, mouse wheel, and process launch
- 🌙 **Dark mode**: automatically follows the system theme
- 🌐 **Bilingual UI**: switch between Chinese and English with one click
- 📝 **Run log**: real-time execution status and error messages
- 💾 **Persistent config**: settings are saved automatically and restored on next launch

## 🖼️ UI Preview

```
┌──────────────────────────────┐
│ MouseClickTool Auto Clicker ●Ready│
├──────────────────────────────┤
│ Action      [Left Click]     │
│ Interval    [100] ms  ☑Random │
│ Click Count [0] (0 = infinite)│
│ Hotkey      [F8] (toggles)   │
│ ☑ Timed Trigger [0h 0m 0s]   │
│ ☑ Record Logs                │
│ ┌─ Log ──────────────────┐  │
│ │ 14:00:00 Started…        │  │
│ └────────────────────────┘  │
│         [   Start   ]       │
└──────────────────────────────┘
```

## 🚀 Quick Start

### Prerequisites

- Rust 1.75+ (install via [rustup.rs](https://rustup.rs))
- Windows 10/11 (other platforms not tested)

### Build & Run

```bash
# Debug build
cargo build

# Release build (recommended: smaller binary, better performance)
cargo build --release

# Run directly
cargo run --release
```

The release binary is at `target/release/mouse-click-tool.exe` and can be copied to any directory and run standalone.

## 📖 Usage

| Setting | Description |
|---------|-------------|
| **Action** | Left click / right click / left hold / right hold / middle click / wheel / launch program / run script |
| **Interval** | Delay between each action in milliseconds (1–600000) |
| **Random jitter** | When enabled, the actual interval fluctuates within ±20% of the set value |
| **Click count** | Maximum number of executions; `0` means unlimited |
| **Global hotkey** | F1–F12 / Home / End; press start/stop to toggle clicking |
| **Timed trigger** | Set h/m/s to auto-start at the specified time |
| **Launch program / script** | Enter an executable path or `.msck` script path after selecting the action |

### Global Hotkeys

While clicking is running, press the registered global hotkey (e.g. `F8`) to **stop at any time** — no need to switch windows.

## 📜 Script Syntax (.msck)

Load a `.msck` script file via "Action = Run Script"; commands are executed line by line:

| Command | Description | Example |
|---------|-------------|---------|
| `title("...")` | Set the window title | `title("Demo")` |
| `delay(ms)` | Wait for a duration | `delay(500)` |
| `left_click(x, y)` | Left-click at coordinates | `left_click(100, 200)` |
| `right_click(x, y)` | Right-click at coordinates | `right_click(300, 400)` |
| `left_click_long(x, y, type)` | Left-click and hold (see type below) | `left_click_long(100, 200, 0)` |
| `right_click_long(x, y, type)` | Right-click and hold | `right_click_long(300, 400, 1)` |
| `mouse_wheel(v)` | Scroll the wheel (positive/negative) | `mouse_wheel(3)` |
| `create_process("path")` | Launch an external program | `create_process("notepad.exe")` |
| `once()` | Run only once (default is loop) | `once()` |
| `exit()` | Exit the script immediately | `exit()` |

> Lines starting with `#` are comments; the `type` parameter controls hold behavior (`0` = press, `1` = release).
> See examples: `Scripts/demo.msck`, `Scripts/demo_en.msck`

## 💾 Configuration

Settings are stored in `mouse_click_tool.json` next to the executable (UTF-8). Delete the file to reset to defaults.

## 🛠️ Tech Stack

- [egui / eframe](https://github.com/emilk/egui) 0.35 — pure Rust GUI (glow renderer)
- [enigo](https://github.com/enigo-rs/enigo) 0.6 — mouse input simulation
- [global-hotkey](https://github.com/tauri-apps/global-hotkey) 0.8 — global hotkeys
- serde / serde_json — configuration serialization

## 📁 Project Structure

```
├── src/
│   ├── main.rs      # GUI and main logic (egui)
│   ├── config.rs    # Config persistence (JSON)
│   └── script.rs    # .msck script parser
├── Scripts/         # Example scripts (.msck)
├── Cargo.toml
└── README.md
```

## 🤝 Contributing

Issues and PRs are welcome.

## 📄 License

[MIT](LICENSE)

---

**Disclaimer**: This tool is intended for legitimate automation of personal workflows only. Do not use it for game cheating or in violation of software terms of service. The user is solely responsible for any consequences of its use.
