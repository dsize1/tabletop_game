//! 技能注册表 + **声明式 SkillDef** + 单一状态源（`active`）。
//!
//! 这是 M4 的核心（§6.3 技能模型从「handler」到「声明」）。关键差异 vs v2：
//! 1. **face 化**：转换技/主动+响应类拆多 face，每个 face 独立声明窗口（治「拒战阴面误写成他人回合」）；
//! 2. **subscription 带 Subject**：事件归属是声明不是守卫（M105 教训，治屯田/燕语在「他人失去牌」误触发）；
//! 3. **授予技一等公民**（O2=A）：`grant_only` + 所有技能（含怀柔/清侧/排异/攻心）都进注册表；
//! 4. **单一状态源**（O4=A）：`active()` = 自带 ∪ granted ∪ 临时，禁止 flags/counters 各自表达「拥有技能」。

use super::event::{EventKind, Subject, Window};

pub mod b0;

/// 技能类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillKind {
    Normal,
    Locked,
    Limited,
    Awaken,
    Lord,
    /// 装备特效（O3=A：武器/防具 = 特殊技能）。
    Equip,
}

/// 钩子（归一 v2 的 30+ 个 `skills::xxx()` 散装函数）。技能只声明它挂了哪些钩子，
/// 内核/牌结算按钩子类型回调，不 match 技能名。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hook {
    /// 濒死钩子（不屈：濒死翻创判定）。
    Dying,
    /// 翻面钩子（解围②：翻回正面移动装备）。
    Flip,
    /// 无懈替代（解围①/看破：装备区牌当无懈）。
    WuxieOpt,
    /// 转化牌（武圣/龙胆/急救/丈八…）。
    Conversion,
    /// 距离修正（马术：-1）。
    Distance(i32),
    /// 【杀】次数覆盖（咆哮：无限）。
    ShaLimit,
    /// 手牌上限修正（血裔：+N）。
    HandLimit(i32),
}

/// 订阅（声明式）：事件 + 主体。handler 首行按 `subject` 校验（不再靠人记）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subscription {
    pub event: EventKind,
    pub subject: Subject,
}

impl Subscription {
    /// 订阅是否匹配该事件（事件类型 + 主体，相对 `owner`）。窗口由 `Face.window` 判定。
    pub fn matches(&self, ev: &super::event::Event, owner: usize) -> bool {
        self.event == ev.kind && self.subject.matches(ev, owner)
    }
}

/// 技能面：独立窗口 + 订阅 + 钩子。默认单 face；转换技/主动+响应类拆多 face。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Face {
    pub window: Window,
    pub subscriptions: &'static [Subscription],
    pub hooks: &'static [Hook],
    /// 是否有主动入口（出牌阶段可 `skill:<code>` 发动）。M4 后续把 bool 升级为条件函数。
    pub can_active: bool,
}

/// 技能定义（声明式；行为 handler 由 `SkillDef` 的 code 在 `skills::handler` 里分发，M4 分批接入）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDef {
    pub code: &'static str,
    pub name: &'static str,
    pub kind: SkillKind,
    pub faces: &'static [Face],
    /// 授予技（一等公民）：不能被选将自带，只能被授予（攻心/怀柔/清侧/排异/弦技…）。
    pub grant_only: bool,
}

/// 技能注册表（B0 起步，逐批追加）。
pub static SKILLS: &[SkillDef] = &[
    // —— B0：据守/解围/不屈/奋激/行殇/放逐/颂威 ——
    SkillDef {
        code: "jushou",
        name: "据守",
        kind: SkillKind::Normal,
        grant_only: false,
        faces: &[Face {
            window: Window::OwnPhase(super::Phase::End),
            subscriptions: &[],
            hooks: &[],
            can_active: true,
        }],
    },
    SkillDef {
        code: "jiewei",
        name: "解围",
        kind: SkillKind::Normal,
        grant_only: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[],
            hooks: &[Hook::WuxieOpt, Hook::Flip],
            can_active: false,
        }],
    },
    SkillDef {
        code: "buqu",
        name: "不屈",
        kind: SkillKind::Locked,
        grant_only: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[],
            hooks: &[Hook::Dying],
            can_active: false,
        }],
    },
    SkillDef {
        code: "fenji",
        name: "奋激",
        kind: SkillKind::Normal,
        grant_only: false,
        faces: &[Face {
            window: Window::AnyTime,
            // 奋激：其他角色的手牌被弃置/获得 → subject = Other（M105 教训的类型级对策）。
            subscriptions: &[
                Subscription { event: EventKind::CardDiscarded, subject: Subject::Other },
                Subscription { event: EventKind::CardGained, subject: Subject::Other },
            ],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "xingshang",
        name: "行殇",
        kind: SkillKind::Normal,
        grant_only: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[Subscription { event: EventKind::Death, subject: Subject::Other }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "fangzhu",
        name: "放逐",
        kind: SkillKind::Normal,
        grant_only: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "songwei",
        name: "颂威",
        kind: SkillKind::Lord,
        grant_only: false,
        faces: &[Face {
            window: Window::AnyTime,
            // 颂威：其他魏势力黑色判定生效 → subject = Other（主公技他人侧）。
            subscriptions: &[Subscription { event: EventKind::JudgeEffective, subject: Subject::Other }],
            hooks: &[],
            can_active: false,
        }],
    },
];

/// 查技能定义。
pub fn def(code: &str) -> Option<&'static SkillDef> {
    SKILLS.iter().find(|s| s.code == code)
}

/// **单一状态源**（O4=A / mechanics `skill-status`）：自带 ∪ granted ∪ 临时。
///
/// 禁止用 flags/turn_counters/marks 各自表达「拥有技能」；所有钩子/视图/合法动作只调这里。
pub fn active(g: &super::Game, seat: usize, code: &str) -> bool {
    let Some(s) = g.seats.get(seat) else {
        return false;
    };
    s.skills.iter().any(|c| c == code) || s.granted.iter().any(|c| c == code)
}

/// 授予技能（唯一入口；授予技一等公民，grant 后 `active()` 立即可查到）。
pub fn grant(g: &mut super::Game, seat: usize, code: &str) {
    let Some(s) = g.seats.get_mut(seat) else {
        return;
    };
    if !s.granted.iter().any(|c| c == code) {
        s.granted.push(code.to_string());
    }
}

/// 技能 handler 分发（`resolve::advance` 用；按 code 找 B0 起的分批 handler）。
pub fn handler(
    code: &str,
) -> Option<fn(&mut super::Game, &super::resolve::Ask) -> Result<super::resolve::Step, String>> {
    b0::handler(code)
}

/// 事件路由：把事件快照按 SKILLS 的订阅（窗口 + subject）路由给技能帧。
/// 先收集匹配（避免借用冲突），再统一压帧。
/// 开局/发牌等 `Lifecycle::Setup` 事件**不路由**（明任开局置任的教训）。
pub fn route_event(g: &mut super::Game, ev: &super::event::Event) {
    if ev.lifecycle == super::event::Lifecycle::Setup {
        return;
    }
    let mut to_push: Vec<(&'static str, usize, serde_json::Value)> = Vec::new();
    for seat in 0..g.seats.len() {
        for skill in SKILLS {
            if !active(g, seat, skill.code) {
                continue;
            }
            for face in skill.faces {
                if !face.window.allows(ev.turn, ev.phase, seat) {
                    continue;
                }
                for sub in face.subscriptions {
                    if sub.matches(ev, seat) {
                        let data = match ev.kind {
                            EventKind::Death => serde_json::json!({ "victim": ev.seat }),
                            EventKind::DamageAfter => serde_json::json!({ "target": ev.by }),
                            _ => serde_json::json!({ "victim": ev.seat }),
                        };
                        to_push.push((skill.code, seat, data));
                        break; // 一次事件一个技能最多触发一次
                    }
                }
            }
        }
    }
    for (code, seat, data) in to_push {
        super::resolve::push_ask(g, code, super::resolve::Stage::Choose, seat, data);
    }
}
