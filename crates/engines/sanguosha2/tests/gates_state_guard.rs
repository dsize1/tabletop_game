//! 门禁：技能 handler 里的「触发阶段字段」必须在 `event.is_some()` 守卫内读取。
//!
//! 背景：同一类缺陷在 B0–B2 反复出现 5 次 —— handler 在**后续阶段**仍按触发阶段的
//! `data["seat"]` / `data["cause"]` 做校验，于是玩家一回答就直接 `Done`（技能"答完就消失"）。
//!
//! 规则（按函数体扫描）：若函数体读取 `data["seat"]` 或 `data["cause"]`，
//! 则函数体内必须出现 `data.get("event").is_some()` 守卫。

use std::fs;
use std::path::Path;

const FILES: &[&str] = &[
    "src/skills/b0.rs",
    "src/skills/b1.rs",
    "src/skills/b1b.rs",
    "src/skills/b2.rs",
    "src/skills/liangyin.rs",
    "src/skills/kongsheng.rs",
    "src/skills/zhenliang.rs",
    "src/skills/b3.rs",
    "src/skills/b4.rs",
    "src/skills/b5.rs",
    "src/skills/b6.rs",
    "src/skills/b7.rs",
    "src/kernel/flow.rs",
];

/// 粗略按 `fn name(..) {` + 花括号配平切出函数体。
fn functions(src: &str) -> Vec<(String, String)> {
    let bytes: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        // 找 `fn ` 且在行首附近（避免注释里的 fn）
        if bytes[i] == 'f' && i + 2 < bytes.len() && &src[byte_to_char(src, i)..][..2] == "fn" {
            let rest: String = bytes[i..].iter().collect();
            if let Some(paren) = rest.find('(') {
                let name_end = i + paren;
                let name: String = bytes[i + 2..name_end].iter().collect::<String>().trim().to_string();
                // 找函数体的第一个 '{'
                let mut j = name_end;
                while j < bytes.len() && bytes[j] != '{' {
                    j += 1;
                }
                if j < bytes.len() {
                    let start = j;
                    let mut depth = 0i32;
                    let mut k = j;
                    while k < bytes.len() {
                        if bytes[k] == '{' {
                            depth += 1;
                        } else if bytes[k] == '}' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        k += 1;
                    }
                    let body: String = bytes[start..=k.min(bytes.len() - 1)].iter().collect();
                    out.push((name, body));
                    i = k;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

fn byte_to_char(src: &str, byte_idx: usize) -> usize {
    src.char_indices().nth(byte_idx).map(|(b, _)| b).unwrap_or(src.len())
}

#[test]
fn gate_state_guard_in_handlers() {
    let mut problems = Vec::new();
    for f in FILES {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(f);
        let src = fs::read_to_string(&path).expect("读源文件");
        for (name, body) in functions(&src) {
            // 只查「多阶段 handler 的前导代码」：真正会泄漏到后续阶段的写法。
            if !body.contains("Step::Next(") && !body.contains("Step::NeedAt(") {
                continue; // 单段询问：data 不会被替换，安全
            }
            let preamble = match body.find("match a.stage") {
                Some(pos) => &body[..pos],
                None => body.as_str(),
            };
            let reads_trigger_field =
                preamble.contains("data[\"seat\"]") || preamble.contains("data[\"cause\"]");
            if !reads_trigger_field {
                continue;
            }
            let guarded = preamble.contains("data.get(\"event\").is_some()")
                || preamble.contains("data[\"event\"]");
            if !guarded {
                problems.push(format!("{f}::{name}"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "以下 handler 读取触发阶段字段但没有 `data.get(\"event\").is_some()` 守卫：\n  {}",
        problems.join("\n  ")
    );
}
