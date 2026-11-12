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
    // p2 手牌被 p1 弃置（口径 B：被其他角色弃置）→ 触发奋激
    let card = g.hand(2)[0].clone();
    g.to_discard_by(1, &card, "scene").unwrap();
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

fn scenario_sanyao_zhiman() -> (String, String, Rec) {
    let mut g = mk(vec![
        seat("p0", "masu", &["sanyao", "zhiman"], "shu", 3),
        seat("p1", "other", &[], "qun", 4),
    ]);
    let mut r = Rec::new();
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    skills::execute_active(&mut g, 0, "skill:sanyao").unwrap();
    r.act(&mut g, "散谣询问（两模式各限一次）", None, None);
    r.check(
        "散谣应询问",
        g.pending.as_ref().and_then(|p| p.skill()) == Some("sanyao"),
    );
    let cost = g.hand(0)[0].clone();
    let victim_card = g.hand(1)[0].clone();
    r.act(&mut g, "选择「体力值最大」模式", Some(0), Some("choose:hp"));
    r.act(&mut g, "弃置成本", Some(0), Some(&format!("choose:{cost}")));
    r.act(&mut g, "选定目标（p1 体力最大）", Some(0), Some("target:1"));
    r.check(
        "造成伤害前先问制蛮",
        g.pending.as_ref().and_then(|p| p.skill()) == Some("zhiman"),
    );
    r.act(&mut g, "发动制蛮（防止伤害）", Some(0), Some("choose:zhiman"));
    r.act(
        &mut g,
        "获得其区域里一张牌",
        Some(0),
        Some(&format!("choose:{victim_card}")),
    );
    r.check("伤害被防止（p1 仍 4 血）", g.seats[1].hp == 4);
    r.check("获得的牌进入马谡手牌", g.hand(0).contains(&victim_card));
    (
        "sanyao-zhiman-chain".into(),
        "散谣 → 制蛮：结算伤害前防止并获得牌".into(),
        r,
    )
}

fn scenario_junxing_no_hand_target() -> (String, String, Rec) {
    let mut g = mk(vec![
        seat("p0", "manchong", &["junxing"], "wei", 4),
        seat("p1", "other", &[], "qun", 4),
    ]);
    // 目标没有手牌
    while let Some(id) = g.hand(1).first().cloned() {
        g.to_discard(&id, "scene").unwrap();
    }
    let mut r = Rec::new();
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    skills::execute_active(&mut g, 0, "skill:junxing").unwrap();
    let cost = g.hand(0)[0].clone();
    r.act(&mut g, "峻刑：选择成本", None, None);
    r.act(&mut g, "弃置 1 张手牌", Some(0), Some(&format!("choose:{cost}")));
    r.act(&mut g, "确认弃置", Some(0), Some("confirm"));
    r.check(
        "目标不要求有手牌（可选 p1）",
        g.pending
            .as_ref()
            .map(|p| p.options().iter().any(|o| o.id == "target:1"))
            .unwrap_or(false),
    );
    r.act(&mut g, "选定无手牌目标", Some(0), Some("target:1"));
    r.check(
        "只能翻面（没有类型不同的手牌可弃）",
        g.pending
            .as_ref()
            .map(|p| p.options().iter().all(|o| o.id != "opt1"))
            .unwrap_or(false),
    );
    r.act(&mut g, "翻面并摸 1 张", Some(1), Some("opt2"));
    r.check("目标已翻面", g.seats[1].flipped);
    r.check("摸与弃置数相同（1 张）", g.hand(1).len() == 1);
    (
        "junxing-no-hand-target".into(),
        "峻刑：无手牌目标只能翻面摸牌".into(),
        r,
    )
}

fn scenario_sidi_freeze() -> (String, String, Rec) {
    // 曹真装备区有红装备（即为费用）；p1 有红/黑各一张手牌
    let base = mk(vec![
        seat("p0", "caozhen", &["sidi"], "wei", 4),
        seat("p1", "other", &[], "qun", 4),
    ]);
    let mut cards: Vec<Card> = base.cards.values().cloned().collect();
    for i in 19..=30 {
        cards.push(Card::new(format!("c{i}"), "闪", Suit::Heart, 2));
    }
    cards.push(Card::equip(
        "c19x",
        "八卦阵",
        Suit::Heart,
        2,
        sanguosha2::kernel::Slot::Armor,
    ));
    cards.push(Card::new("c21", "闪", Suit::Heart, 3));
    cards.push(Card::new("c22", "杀", Suit::Spade, 4));
    let mut g = Game::new(base.seats.clone(), cards);
    g.to_equip(&"c19x".to_string(), 0, sanguosha2::kernel::Slot::Armor)
        .unwrap();
    g.to_hand(&"c21".to_string(), 1).unwrap();
    g.to_hand(&"c22".to_string(), 1).unwrap();
    let mut r = Rec::new();
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    r.act(&mut g, "司敌：其他角色出牌阶段开始", None, None);
    r.check(
        "装备区牌本身即可作为费用（可发动）",
        g.pending.as_ref().and_then(|p| p.skill()) == Some("sidi"),
    );
    r.act(
        &mut g,
        "弃置装备区红色牌（封锁红色）",
        Some(0),
        Some("choose:c19x"),
    );
    r.check("红色手牌被冻结", !g.can_use_card(1, &"c21".to_string()));
    r.check("黑色手牌不受影响", g.can_use_card(1, &"c22".to_string()));
    (
        "sidi-freeze-color".into(),
        "司敌：手牌冻结（阶段级）".into(),
        r,
    )
}

fn scenario_xiansi() -> (String, String, Rec) {
    let mut g = mk(vec![
        seat("p0", "liufeng", &["xiansi"], "shu", 4),
        seat("p1", "other", &[], "qun", 4),
    ]);
    let mut r = Rec::new();
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    r.act(&mut g, "准备阶段：陷嗣询问", None, None);
    r.check(
        "陷嗣应询问",
        g.pending.as_ref().and_then(|p| p.skill()) == Some("xiansi"),
    );
    r.act(&mut g, "选择一名角色", Some(0), Some("target:1"));
    r.act(&mut g, "确认", Some(0), Some("confirm"));
    let card = g
        .pending
        .as_ref()
        .and_then(|p| p.options().first().map(|o| o.id.clone()))
        .unwrap_or_default();
    r.act(&mut g, "取走其一张牌为「逆」", Some(0), Some(&card));
    r.check("「逆」计数 = 1", skills::ni(&g, 0) == 1);
    r.check(
        "「逆」按移出游戏处理（void 区有牌）",
        g.ids_in(|z| *z == Zone::Void).len() == 1,
    );
    (
        "xiansi-ni-void".into(),
        "陷嗣：「逆」移出游戏".into(),
        r,
    )
}

fn scenario_xingshuai() -> (String, String, Rec) {
    let mut g = mk(vec![
        seat("p0", "caorui", &["xingshuai"], "wei", 3),
        seat("p1", "weially", &[], "wei", 4),
        seat("p2", "qun", &[], "qun", 4),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    g.seats[0].hp = 1;
    let mut r = Rec::new();
    flow::damage(&mut g, Some(2), 0, 1); // 1 → 0 濒死
    r.act(&mut g, "曹叡濒死（求桃从左手方开始）", None, None);
    // 其他角色（有手牌）逐个被询问「救不救」——I1：有手牌就必须显式询问（唯一实质动作=放弃）。
    // v1 口径：无手牌才自动跳过，所以这里要按座位顺序把非濒死者的询问全部回应掉。
    let mut guard = 0;
    while g.pending.as_ref().map(|p| p.actor()).is_some_and(|a| a != 0) && guard < 8 {
        guard += 1;
        let actor = g.pending.as_ref().map(|p| p.actor()).unwrap_or(0);
        r.act(&mut g, "其他角色放弃救援", Some(actor), Some("pass"));
    }
    r.check(
        "求到自己时可选择发动兴衰",
        g.pending
            .as_ref()
            .map(|p| p.actor() == 0 && p.options().iter().any(|o| o.id == "xingshuai"))
            .unwrap_or(false),
    );
    let wei_hp = g.seats[1].hp;
    r.act(&mut g, "发动兴衰", Some(0), Some("xingshuai"));
    r.check(
        "先逐个询问魏势力角色",
        g.pending.as_ref().map(|p| p.actor()) == Some(1),
    );
    r.act(&mut g, "魏势力角色支援（回复 1 点）", Some(1), Some("opt1"));
    r.check("曹叡脱离濒死（0 → 1）", g.seats[0].hp == 1);
    r.check(
        "支援者在濒死结算后受到 1 点伤害",
        g.seats[1].hp == wei_hp - 1,
    );
    (
        "xingshuai-lord-dying".into(),
        "兴衰：魏势力施救 → 脱困后支援者受伤".into(),
        r,
    )
}

fn scenario_zanhui() -> (String, String, Rec) {
    let mut g = mk(vec![
        seat("p0", "sunluban", &["zanhui"], "wu", 4),
        seat("p1", "target", &[], "qun", 4),
        seat("p2", "helper", &[], "qun", 4),
    ]);
    let mut r = Rec::new();
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    // 用【杀】指定唯一目标
    g.use_card(0, &"c1".to_string(), vec![1]);
    r.act(&mut g, "使用【杀】指定唯一目标 → 谮毁询问", None, None);
    r.check(
        "谮毁应询问",
        g.pending.as_ref().and_then(|p| p.skill()) == Some("zanhui"),
    );
    r.act(&mut g, "选择协助者", Some(0), Some("target:2"));
    r.check(
        "由协助者二选一",
        g.pending.as_ref().map(|p| p.actor()) == Some(2),
    );
    r.act(&mut g, "成为额外目标", Some(2), Some("opt2"));
    r.check(
        "追加额外目标已记录",
        g.counter("turn.zanhui_extra_target") == 1,
    );
    (
        "zanhui-unique-target".into(),
        "谮毁：唯一目标 → 协助者成为额外目标".into(),
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
        scenario_sanyao_zhiman(),
        scenario_junxing_no_hand_target(),
        scenario_sidi_freeze(),
        scenario_xiansi(),
        scenario_xingshuai(),
        scenario_zanhui(),
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
