//! 自定义脚本引擎。解析并执行 `*.msck` 脚本，对应原 C# 脚本解析逻辑。
//! 语法：`command(args)`，以 `#` 开头的行或不含 `(` 的行视为注释/忽略。

/// 单条脚本命令
#[derive(Debug, Clone)]
pub enum ScriptCmd {
    Title(String),
    Delay(u64),
    LeftClick(Option<(i32, i32)>),
    RightClick(Option<(i32, i32)>),
    LeftLong(Option<(i32, i32)>, bool), // bool = true 表示按下(1)，false 表示松开(0)
    RightLong(Option<(i32, i32)>, bool),
    Wheel(i32),
    Process(String),
    Once,
    Exit,
}

/// 解析整个脚本内容为命令列表
pub fn parse_script(content: &str) -> Vec<ScriptCmd> {
    let mut cmds = Vec::new();
    for line in content.lines() {
        let raw = line.trim().trim_end_matches(')');
        let r_index = match raw.find('(') {
            Some(i) => i,
            None => continue,
        };
        let name = raw[..r_index].trim().to_lowercase();
        let args_str = raw[r_index + 1..].trim();
        if name.starts_with('#') {
            continue;
        }
        let args: Vec<&str> = args_str.split(',').map(|s| s.trim()).collect();
        match name.as_str() {
            "delay" | "sleep" => {
                if let Ok(ms) = args_str.parse::<u64>() {
                    cmds.push(ScriptCmd::Delay(ms));
                }
            }
            "title" => {
                let t = args_str.trim_matches('"').to_string();
                cmds.push(ScriptCmd::Title(t));
            }
            "left_click" => cmds.push(ScriptCmd::LeftClick(parse_pos(&args))),
            "right_click" => cmds.push(ScriptCmd::RightClick(parse_pos(&args))),
            "left_click_long" => {
                let press = args.get(2).map_or(false, |a| a.contains('1'));
                cmds.push(ScriptCmd::LeftLong(parse_pos(&args), press));
            }
            "right_click_long" => {
                let press = args.get(2).map_or(false, |a| a.contains('1'));
                cmds.push(ScriptCmd::RightLong(parse_pos(&args), press));
            }
            "mouse_wheel" => {
                if let Some(v) = args.first() {
                    if let Ok(val) = v.parse::<i32>() {
                        cmds.push(ScriptCmd::Wheel(val));
                    }
                }
            }
            "create_process" => {
                cmds.push(ScriptCmd::Process(args_str.trim_matches('"').to_string()));
            }
            "once" | "break" => cmds.push(ScriptCmd::Once),
            "exit" | "quit" => cmds.push(ScriptCmd::Exit),
            _ => {}
        }
    }
    cmds
}

/// 解析坐标参数：两个参数均非空且可解析为整数时返回 Some((x,y))，否则 None（使用当前坐标）
fn parse_pos(args: &[&str]) -> Option<(i32, i32)> {
    if args.len() > 1 {
        let x = args[0].trim();
        let y = args[1].trim();
        if x.eq_ignore_ascii_case("null") || y.eq_ignore_ascii_case("null") {
            return None;
        }
        if let (Ok(px), Ok(py)) = (x.parse::<i32>(), y.parse::<i32>()) {
            return Some((px, py));
        }
    }
    None
}
