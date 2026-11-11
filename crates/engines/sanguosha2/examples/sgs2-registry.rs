//! v2 技能注册表快照（harness 用）：把 `skills::SKILLS` 导出为稳定 JSON。
//!
//! 用途：
//! - `apps/scripts/check-sgs2-registry.mjs` 校验注册表唯一性/覆盖率/转换技清单；
//! - B8–B20 迁移时快速看「哪些技能已注册、触发点/窗口/主动技」。
//!
//! 用法：cargo run -q -p sanguosha2 --example sgs2-registry -- --json

use sanguosha2::skills;

fn window_label(w: &sanguosha2::kernel::event::Window) -> String {
    format!("{w:?}")
}

fn main() {
    let json_out = std::env::args().any(|a| a == "--json");
    let mut rows = Vec::new();
    for d in skills::SKILLS {
        rows.push(serde_json::json!({
            "code": d.code,
            "name": d.name,
            // 技能类型（normal/locked/limited/awaken/lord）与主公技的「他人侧」模式。
            "kind": format!("{:?}", d.kind).to_lowercase(),
            "lord_side": if d.kind == skills::SkillKind::Lord {
                Some(skills::lord::others_side(d.code).label())
            } else {
                None
            },
            "triggers": d.triggers.iter().map(|k| format!("{k:?}")).collect::<Vec<_>>(),
            "window": window_label(&d.window),
            "priority": d.priority,
            "active": d.can_active.is_some(),
            "dying": d.dying_hook.is_some(),
            "flip": d.flip_hook.is_some(),
            "wuxie_opts": d.wuxie_opts.is_some(),
            "wuxie_use": d.wuxie_use.is_some(),
            "target_blocker": d.target_blocker.is_some(),
            "yang_window": d.yang_window.as_ref().map(window_label),
            "yin_window": d.yin_window.as_ref().map(window_label),
            "transform": skills::TRANSFORM_SKILLS.contains(&d.code),
        }));
    }
    let payload = serde_json::json!({
        "engine": "sanguosha2",
        "count": rows.len(),
        "transform_skills": skills::TRANSFORM_SKILLS,
        "skills": rows,
    });
    if json_out {
        println!("{}", serde_json::to_string(&payload).unwrap());
    } else {
        println!("v2 注册技能 {} 个", rows.len());
        for d in skills::SKILLS {
            println!(
                "  {:<12} {:<8} active={} triggers={:?} window={:?}",
                d.code,
                d.name,
                d.can_active.is_some(),
                d.triggers.iter().map(|k| format!("{k:?}")).collect::<Vec<_>>(),
                d.window
            );
        }
    }
}
