//! 可复现场景（人工抽检用）：跑 6 个关键裁定场景，输出带步骤的 JSON。
//!
//! 用法：`cargo run -q --manifest-path crates/Cargo.toml -p sanguosha2 --example sgs2-scenes`
//! 由 `apps/scripts/sgs2-scenes.mjs` 渲染成 `docs/games/sanguosha/scenes/index.html`（浏览器直接打开）。

use serde_json::{json, Value};
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Suit, Zone};
use sanguosha2::skills;

fn seat(name: &str, general: &str, skills: &[&str], kingdom: &str, hp: i32) -> Seat {
    let mut s = Seat::new(name, general, skills, hp);
    s.kingdom = kingdom.to_string();
    s
}

fn mk(specs: Vec<Seat>) -> Game {
    let n = specs.len();
    let names = ["杀", "闪", "桃", "杀", "无懈可击", "杀", "闪", "桃", "杀", "决斗"];
    let mut cards = Vec::new();
    for i in 1..=20 {
        cards.push(Card::new(
            format!("c{i}"),
            names[(i - 1) % names.len()],
            if i % 2 == 0 { Suit::Heart } else { Suit::Spade },
            (i % 13) as u8,
        ));
    }
    let mut g = Game::new(specs, cards);
    for s in 0..n {
        for j in 0..3 {
            g.to_hand(&format!("c{}", s * 3 + j + 1), s).unwrap();
        }
    }
    g
}

fn snap(g: &Game) -> Value {
    json!({
        "turn": g.turn,
        "phase": format!("{:?}", g.phase),
        "hp": g.seats.iter().map(|s| s.hp).collect::<Vec<_>>(),
        "hands": (0..g.seats.len()).map(|i| g.hand(i).len()).collect::<Vec<_>>(),
        "void": g.ids_in(|z| *z == Zone::Void).len(),
        "discard": g.discard_pile().len(),
        "kongsheng": g.mark_cards(0, "kongsheng").len(),
    })
}

struct Rec {
    steps: Vec<Value>,
    checks: Vec<Value>,
}

impl Rec {
    fn new() -> Self {
        Self {
            steps: Vec::new(),
            checks: Vec::new(),
        }
    }

    fn check(&mut self, desc: &str, ok: bool) {
        self.checks.push(json!({ "desc": desc, "ok": ok }));
    }

    /// 记录并执行一个动作（`None` 表示只推进）。
    fn act(&mut self, g: &mut Game, label: &str, actor: Option<usize>, action: Option<&str>) {
        let before = g.pending.as_ref().map(|p| {
            json!({
                "skill": p.skill(),
                "actor": p.actor(),
                "options": p.options().iter().map(|o| o.id.clone()).collect::<Vec<_>>(),
            })
        });
        let mut err = Value::Null;
        match (actor, action) {
            (Some(a), Some(act)) => {
                if let Err(e) = submit(g, a, act) {
                    err = json!(e);
                }
            }
            _ => {
                if let Err(e) = advance(g) {
                    err = json!(e);
                }
            }
        }
        let after = g.pending.as_ref().map(|p| {
            json!({
                "skill": p.skill(),
                "actor": p.actor(),
                "options": p.options().iter().map(|o| o.id.clone()).collect::<Vec<_>>(),
                "escape": p.escape_id(),
            })
        });
        self.steps.push(json!({
            "label": label,
            "actor": actor,
            "action": action,
            "pending_before": before,
            "error": err,
            "pending_after": after,
            "state": snap(g),
            "log": g.log.iter().rev().take(2).map(|l| l.text.clone()).collect::<Vec<_>>(),
            "presentation": g.presentation.iter().rev().take(2).map(|p| p.text()).collect::<Vec<_>>(),
        }));
    }
}

fn scenario_jushou() -> (String, String, Rec) {
    let mut g = mk(vec![
        seat("p0", "caoren", &["jushou", "jiewei"], "wei", 4),
        seat("p1", "other", &[], "qun", 4),
    ]);
    g.seats[0].flipped = true; // 背面朝上：据守会翻回正面 → 触发解围②
    let mut r = Rec::new();
    begin_phase(&mut g, 0, Phase::End).unwrap();
    r.act(&mut g, "结束阶段：据守询问", None, None);
    r.check("据守应询问", g.pending.as_ref().and_then(|p| p.skill()) == Some("jushou"));
    r.act(&mut g, "发动据守（翻面）", Some(0), Some("choose:jushou"));
    r.check(
        "翻回正面后应先结算解围②",
        g.pending.as_ref().and_then(|p| p.skill()) == Some("jiewei"),
    );
    r.check("据守把背面翻回正面（取反）", !g.seats[0].flipped);
    r.act(&mut g, "不发动解围", Some(0), Some("pass"));
    r.check(
        "解围结束后继续据守（摸 4 后询问弃/使用）",
        g.pending.as_ref().and_then(|p| p.skill()) == Some("jushou"),
    );
    (
        "jushou-flip-jiewei".into(),
        "据守翻回正面 → 先结算解围②".into(),
        r,
    )
}

fn scenario_buqu() -> (String, String, Rec) {
    let mut g = mk(vec![
        seat("p0", "zhoutai", &["buqu"], "wu", 4),
        seat("p1", "other", &[], "qun", 4),
    ]);
    let mut r = Rec::new();
    g.seats[0].hp = 0;
    flow::start_dying(&mut g, 0, None);
    r.act(&mut g, "进入濒死（不屈自动判定）", None, None);
    r.check("不屈回复至 1 点体力", g.seats[0].hp == 1);
    r.check(
        "「创」移出游戏（void=1）",
        g.ids_in(|z| *z == Zone::Void).len() == 1,
    );
    (
        "buqu-chuang-void".into(),
        "不屈：判定牌 → 「创」移出游戏".into(),
        r,
    )
}

fn scenario_fenji() -> (String, String, Rec) {
    let mut g = mk(vec![
        seat("p0", "zhoutai", &["fenji"], "wu", 4),
        seat("p1", "other", &[], "qun", 4),
        seat("p2", "victim", &[], "qun", 4),
    ]);
    g.seats[0].hp = 1;
    let mut r = Rec::new();
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    // p2 手牌被弃置（回合外）→ 触发奋激
    let card = g.hand(2)[0].clone();
    g.to_discard(&card, "scene").unwrap();
    let p2_before = g.hand(2).len();
    r.act(&mut g, "回合外弃牌 → 奋激询问", None, None);
    r.check("奋激应询问", g.pending.as_ref().and_then(|p| p.skill()) == Some("fenji"));
    r.act(&mut g, "发动奋激（失去 1 点体力）", Some(0), Some("choose:fenji"));
    r.check(
        "失去体力后进入濒死（先救）",
        g.pending.as_ref().and_then(|p| p.skill()) == Some("dying"),
    );
    let heal = g.pending.as_ref().unwrap().options()[0].id.clone();
    r.act(&mut g, "濒死求桃", Some(0), Some(&heal));
    r.check("脱困后体力为 1", g.seats[0].hp == 1);
    r.check(
        "脱困后才令其摸 2（p2 手牌 +2）",
        g.hand(2).len() == p2_before + 2,
    );
    (
        "fenji-after-dying".into(),
        "奋激：脱困后才摸 2".into(),
        r,
    )
}

fn scenario_juzhan() -> (String, String, Rec) {
    let mut g = mk(vec![
        seat("p0", "yanyan", &["juzhan"], "shu", 4),
        seat("p1", "other", &[], "qun", 4),
    ]);
    let mut r = Rec::new();
    // 阳：他人回合内
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    g.use_card(1, &"c4".to_string(), vec![0]);
    r.act(&mut g, "他人回合：用杀指定严颜 → 阳面", None, None);
    r.check(
        "阳面（他人回合内）触发",
        g.pending.as_ref().and_then(|p| p.skill()) == Some("juzhan"),
    );
    let opts = g
        .pending
        .as_ref()
        .map(|p| p.options().iter().any(|o| o.id == "choose:juzhan"))
        .unwrap_or(false);
    r.check("阳面提供发动选项", opts);
    r.act(&mut g, "不发动（pass）", Some(0), Some("pass"));
    // 阴：自己回合内
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    g.use_card(0, &"c1".to_string(), vec![1]);
    r.act(&mut g, "自己回合：用杀指定他人 → 阴面", None, None);
    r.check(
        "阴面（自己回合内）触发",
        g.pending.as_ref().and_then(|p| p.skill()) == Some("juzhan"),
    );
    (
        "juzhan-two-windows".into(),
        "拒战：阴阳两面各自窗口".into(),
        r,
    )
}

fn scenario_yili() -> (String, String, Rec) {
    let mut g = mk(vec![
        seat("p0", "luji", &["yili"], "wu", 4),
        seat("p1", "other", &[], "qun", 4),
    ]);
    g.seats[0].hp = 1;
    let mut r = Rec::new();
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    r.act(&mut g, "出牌阶段开始：遗礼询问", None, None);
    r.check("遗礼应询问", g.pending.as_ref().and_then(|p| p.skill()) == Some("yili"));
    r.act(&mut g, "选择失去 1 点体力", Some(0), Some("choose:yili:hp"));
    r.check(
        "代价致濒死：先进入濒死结算",
        g.pending.as_ref().and_then(|p| p.skill()) == Some("dying"),
    );
    let heal = g.pending.as_ref().unwrap().options()[0].id.clone();
    r.act(&mut g, "濒死求桃", Some(0), Some(&heal));
    r.check("救回后体力 1", g.seats[0].hp == 1);
    r.check(
        "脱困后才询问给「橘」",
        g.pending.as_ref().and_then(|p| p.skill()) == Some("yili"),
    );
    (
        "yili-dying-defer".into(),
        "遗礼：濒死救回后才给「橘」".into(),
        r,
    )
}

fn scenario_lijun() -> (String, String, Rec) {
    let mut g = mk(vec![
        seat("p0", "sunliang", &["lijun"], "wu", 4),
        seat("p1", "wugeneral", &[], "wu", 4),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    let mut r = Rec::new();
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    let hand_before = g.hand(1).len();
    g.use_card(1, &"c4".to_string(), vec![0]);
    r.act(&mut g, "吴势力用杀 → 立军询问", None, None);
    r.check(
        "先问使用者是否交出",
        g.pending.as_ref().and_then(|p| p.skill().map(|s| (s, p.actor()))) == Some(("lijun", 1)),
    );
    r.act(&mut g, "交杀给孙亮", Some(1), Some("choose:lijun"));
    r.check("杀进入孙亮手牌", g.hand(0).contains(&"c4".to_string()));
    r.check(
        "第二段由孙亮决定（不自动摸牌）",
        g.pending.as_ref().map(|p| p.actor()) == Some(0) && g.hand(1).len() == hand_before - 1,
    );
    r.act(&mut g, "孙亮选择：摸 1 + 杀次数 +1", Some(0), Some("choose:lijun_bonus"));
    r.check("孙亮选择后才摸 1", g.hand(1).len() == hand_before);
    r.check(
        "杀次数加成已记录",
        g.counter("turn.lijun_sha_bonus") == 1,
    );
    (
        "lijun-second-choice".into(),
        "立军：孙亮二次选择".into(),
        r,
    )
}

fn main() {
    let scenes = vec![
        scenario_jushou(),
        scenario_buqu(),
        scenario_fenji(),
        scenario_juzhan(),
        scenario_yili(),
        scenario_lijun(),
    ];
    let mut out = Vec::new();
    for (id, name, rec) in scenes {
        let ok = rec.checks.iter().all(|c| c["ok"] == json!(true));
        out.push(json!({
            "id": id,
            "name": name,
            "ok": ok,
            "checks": rec.checks,
            "steps": rec.steps,
        }));
    }
    let all_ok = out.iter().all(|s| s["ok"] == json!(true));
    println!(
        "{}",
        serde_json::to_string(&json!({
            "generated_at": "2026-11-06",
            "engine": "sanguosha2 (spike)",
            "all_ok": all_ok,
            "scenes": out,
        }))
        .unwrap()
    );
    let _ = skills::SKILLS.len();
}
