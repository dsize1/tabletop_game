//! B8 技能门禁（8 将 / 17 技能 + 4 条授予弦技）。

use sanguosha3::kernel::resolve::{self, Ask, Stage, Step};
use sanguosha3::kernel::skills::b8;
use sanguosha3::kernel::{flow, gear, skills, Card, DamageKind, DamageSpec, FreezeUntil, Game, Identity, Phase, Seat, Slot, SourceKind, Suit, Zone};

fn seat_with(id: usize, list: &[&str], kingdom: &str) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        skills: list.iter().map(|s| s.to_string()).collect(),
        kingdom: kingdom.to_string(),
        ..Default::default()
    }
}

fn game(n: usize, s0: &[&str]) -> Game {
    let mut seats: Vec<Seat> = (0..n).map(|i| seat_with(i, &[], "shu")).collect();
    seats[0] = seat_with(0, s0, "shu");
    Game { seats, ..Default::default() }
}

fn add_suit(g: &mut Game, id: &str, def: &str, zone: Zone, suit: Suit, rank: u8) {
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(
        id.to_string(),
        Card { id: id.to_string(), def: def.to_string(), suit, rank },
    );
}

fn add(g: &mut Game, id: &str, def: &str, zone: Zone) {
    add_suit(g, id, def, zone, Suit::Club, 7);
}

fn add_deck(g: &mut Game, n: usize) {
    for i in 0..n {
        let id = format!("d{i}");
        add(g, &id, "shan", Zone::Draw);
        g.deck.push(id);
    }
}

fn data(pairs: &[(&str, serde_json::Value)]) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    serde_json::Value::Object(m)
}

fn call(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value) -> Step {
    let h = skills::handler(skill).expect("handler");
    h(g, &Ask { skill: skill.to_string(), stage: Stage::Choose, actor, data: d })
        .expect("不应报错")
}

fn pending_of(step: &Step) -> &sanguosha3::kernel::pending::Pending {
    match step {
        Step::Need(p) | Step::NeedAt(_, p) | Step::NeedWith(_, _, p) => p,
        _ => panic!("期望待办，实际 {step:?}"),
    }
}

fn run_steps(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value, answers: &[&str]) -> Step {
    let mut data = d;
    let mut step = call(g, skill, actor, data.clone());
    for ans in answers {
        loop {
            match &step {
                Step::Next(_, next) => {
                    data = next.clone();
                    step = call(g, skill, actor, data.clone());
                }
                Step::Need(_) | Step::NeedAt(_, _) => break,
                Step::NeedWith(_, next, _) => {
                    data = next.clone();
                    break;
                }
                other => panic!("期望待办，实际 {other:?}"),
            }
        }
        let p = pending_of(&step);
        assert!(
            p.has_option(ans),
            "选项 {ans} 不在 {:?}",
            p.options().iter().map(|o| o.id.clone()).collect::<Vec<_>>()
        );
        g.answer = Some(ans.to_string());
        g.answer_skill = Some(skill.to_string());
        step = call(g, skill, actor, data.clone());
    }
    step
}

fn drain(g: &mut Game, skill: &str, actor: usize, step: Step) -> Step {
    let mut step = step;
    loop {
        match step {
            Step::Next(_, data) => step = call(g, skill, actor, data),
            other => return other,
        }
    }
}

fn options(g: &Game) -> Vec<String> {
    g.pending
        .as_ref()
        .map(|p| p.options().iter().map(|o| o.id.clone()).collect())
        .unwrap_or_default()
}

fn submit(g: &mut Game, seat: usize, option: &str) -> Result<(), String> {
    resolve::submit(g, seat, option)
}

fn use_card(g: &mut Game, seat: usize, id: &str) {
    g.pending = None;
    sanguosha3::kernel::card_play::begin_use(g, seat, id, false).expect("使用");
    resolve::advance(g).expect("推进");
}

// ---------------------------------------------------------------------------
// 忠鉴 / 才识
// ---------------------------------------------------------------------------

#[test]
fn zhongjian_same_color_and_rank_expands_to_twice() {
    let mut g = game(2, &["zhongjian"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_suit(&mut g, "m", "sha", Zone::Hand(0), Suit::Heart, 7);
    add_suit(&mut g, "t1", "tao", Zone::Hand(1), Suit::Heart, 7);
    let step = run_steps(&mut g, "zhongjian", 0, data(&[]), &["t:1", "card:m", "card:t1", "draw"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.counter("turn.0.zhongjian_twice"), 1, "同点 → 本回合限两次");
    assert_eq!(g.counter("turn.0.zhongjian_penalty"), 0);
    assert_eq!(g.hand_size(0), 1, "同色 → 摸一张");
}

#[test]
fn zhongjian_no_match_reduces_hand_limit() {
    let mut g = game(2, &["zhongjian"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_suit(&mut g, "m", "sha", Zone::Hand(0), Suit::Heart, 7);
    add_suit(&mut g, "t1", "tao", Zone::Hand(1), Suit::Spade, 9);
    let s = run_steps(&mut g, "zhongjian", 0, data(&[]), &["t:1", "card:m", "card:t1"]);
    let step = drain(&mut g, "zhongjian", 0, s);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.counter("turn.0.zhongjian_penalty"), 1, "皆不包含 → 手牌上限 -1");
    assert_eq!(g.hand_limit(0), 3);
}

#[test]
fn caishi_heal_blocks_self_target_cards() {
    let mut g = game(2, &["caishi"]);
    g.turn = 0;
    g.phase = Phase::Draw;
    g.seats[0].hp = 2;
    add(&mut g, "t", "tao", Zone::Hand(0));
    let step = run_steps(&mut g, "caishi", 0, data(&[]), &["opt2"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[0].hp, 3);
    // 本回合不能对自己使用牌（桃/装备）。
    assert!(!sanguosha3::kernel::card_play::can_use_now(&g, 0, sanguosha3::kernel::cards::def("tao").unwrap()));
    // 反例：满体力时没有 opt2。
    let mut g2 = game(2, &["caishi"]);
    g2.turn = 0;
    g2.phase = Phase::Draw;
    let step = call(&mut g2, "caishi", 0, data(&[]));
    let p = pending_of(&step);
    assert!(!p.has_option("opt2"));
}

// ---------------------------------------------------------------------------
// 福绵 / 怠宴
// ---------------------------------------------------------------------------

#[test]
fn fumian_draw_bonus_and_switch_increments() {
    let mut g = game(2, &["fumian"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    let step = run_steps(&mut g, "fumian", 0, data(&[]), &["opt1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(skills::draw_bonus(&mut g, 0), 1, "摸牌阶段多摸 1");
    // 下回合切换选项 → N+1。
    g.turn = 0;
    g.phase = Phase::Prepare;
    g.set_counter("turn.0.fumian_reset", 0);
    let step = run_steps(&mut g, "fumian", 0, data(&[]), &["opt2"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.counter("turn.0.fumian_extra"), 2, "切换后 N=2");
}

#[test]
fn fumian_grants_extra_targets_to_red_trick() {
    let mut g = game(3, &["fumian"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.set_counter("turn.0.fumian_extra", 1);
    add_suit(&mut g, "w", "wuzhong_shengyou", Zone::Hand(0), Suit::Heart, 3);
    use_card(&mut g, 0, "w");
    assert_eq!(g.counter("turn.0.fumian_extra_used"), 1, "本回合限一次已消耗");
}

#[test]
fn daiyan_first_heart_basic_and_repeat_target_loses_hp() {
    let mut g = game(2, &["daiyan"]);
    g.turn = 0;
    g.phase = Phase::End;
    add_suit(&mut g, "h", "tao", Zone::Draw, Suit::Heart, 5);
    add_suit(&mut g, "s", "sha", Zone::Draw, Suit::Spade, 6);
    g.deck.push("s".into());
    g.deck.push("h".into());
    let step = run_steps(&mut g, "daiyan", 0, data(&[]), &["t:1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.hand(1).contains(&"h".to_string()), "获得红桃基本牌");
    // 上一回合同一目标 → 失去 1 点体力。
    g.set_counter("daiyan.last.0", 2);
    let step = run_steps(&mut g, "daiyan", 0, data(&[]), &["t:1"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.seats[1].hp, 3);
}

// ---------------------------------------------------------------------------
// 问卦 / 伏诛
// ---------------------------------------------------------------------------

#[test]
fn wengua_other_side_places_card_and_draws() {
    let mut g = game(2, &[]);
    g.seats[1].skills = vec!["wengua".to_string()];
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "d1", "tao", Zone::Draw);
    add(&mut g, "d2", "tao", Zone::Draw);
    g.deck.push("d1".into());
    g.deck.push("d2".into());
    assert!(b8::other_side_actions(&g, 0).contains(&"wengua"));
    b8::start_other(&mut g, 0, "wengua").expect("发起问卦");
    resolve::advance(&mut g).expect("推进");
    submit(&mut g, 0, "card:a").expect("交牌");
    submit(&mut g, 1, "opt1").expect("置顶");
    assert_eq!(g.counter("turn.0.wengua"), 1, "每名角色出牌阶段限一次");
    assert_eq!(g.deck.last().map(|s| s.as_str()), Some("a"), "置顶");
    assert_eq!(g.hand_size(0), 1, "从牌堆底摸一张");
    assert_eq!(g.hand_size(1), 1, "徐氏也从牌堆底摸一张");
}

#[test]
fn fuzhu_uses_all_sha_from_deck() {
    let mut g = game(2, &["fuzhu"]);
    g.seats[1].male = true;
    g.turn = 1;
    g.phase = Phase::End;
    add(&mut g, "k1", "sha", Zone::Draw);
    add(&mut g, "k2", "sha", Zone::Draw);
    add(&mut g, "x", "tao", Zone::Draw);
    g.deck.push("x".into());
    g.deck.push("k1".into());
    g.deck.push("k2".into());
    // 真实链路：压技能帧 → 推进（技能帧会连续使用牌堆中的【杀】）。
    resolve::push_ask(&mut g, "fuzhu", Stage::Choose, 0, data(&[("step", "fire".into()), ("seat", 1.into())]));
    resolve::advance(&mut g).expect("询问");
    submit(&mut g, 0, "t:1").expect("确认伏诛");
    assert_eq!(g.seats[1].hp, 2, "两张【杀】各 1 点伤害");
    assert!(g.deck.iter().all(|id| id != "k1" && id != "k2"), "牌堆【杀】已用尽");
}

// ---------------------------------------------------------------------------
// 守玺 / 惠民
// ---------------------------------------------------------------------------

#[test]
fn shouxi_declares_name_and_makes_slash_ineffective() {
    let mut g = game(2, &["shouxi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "s", "sha", Zone::Hand(0));
    let step = run_steps(
        &mut g,
        "shouxi",
        0,
        data(&[("by", 1.into()), ("card", "s".into())]),
        &["use:sha", "opt2"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(
        g.use_opts("s").intercept,
        Some(sanguosha3::kernel::ShaIntercept::Immune)
    );
    // 反例：同牌名本回合只能声明一次。
    let step = call(&mut g, "shouxi", 0, data(&[("by", 1.into()), ("card", "s".into())]));
    assert!(!pending_of(&step).has_option("use:sha"));
}

#[test]
fn huimin_draws_x_and_distributes_by_seat_order() {
    let mut g = game(3, &["huimin"]);
    g.turn = 0;
    g.phase = Phase::End;
    add_deck(&mut g, 10);
    g.seats[0].hp = 4;
    // 手牌数 < 体力值：0 号 0 张、1 号 0 张、2 号 0 张 → X=3。
    let step = run_steps(
        &mut g,
        "huimin",
        0,
        data(&[]),
        &["use", "t:0", "card:d9", "card:d8", "card:d7"],
    );
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.hand_size(0), 1, "自己获得一张");
    assert_eq!(g.hand_size(1), 1);
    assert_eq!(g.hand_size(2), 1);
}

// ---------------------------------------------------------------------------
// 清弦 / 绝响
// ---------------------------------------------------------------------------

#[test]
fn qingxian_on_damage_makes_source_lose_hp_and_use_equip() {
    let mut g = game(2, &["qingxian"]);
    add(&mut g, "e", "qinggang_jian", Zone::Draw);
    g.deck.push("e".into());
    let step = run_steps(&mut g, "qingxian", 0, data(&[("by", 1.into())]), &["use", "opt1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[1].hp, 3, "来源失去 1 点体力");
    assert!(gear::equip_in(&g, 1, Slot::Weapon).is_some(), "随机使用装备");
}

#[test]
fn qingxian_silent_while_someone_dying() {
    let mut g = game(2, &["qingxian"]);
    g.seats[1].hp = 0;
    assert!(matches!(call(&mut g, "qingxian", 0, data(&[("by", 1.into())])), Step::Done));
}

#[test]
fn juexiang_grants_random_xian_and_protects_from_club() {
    let mut g = game(2, &["juexiang"]);
    g.seats[0].alive = false;
    b8::on_death(&mut g, 0);
    resolve::advance(&mut g).expect("延迟询问");
    submit(&mut g, 0, "t:1").expect("选目标");
    let granted = ["hexian", "rouxian", "liexian", "jixian"]
        .iter()
        .any(|c| g.seats[1].granted.iter().any(|x| x == c));
    assert!(granted, "随机获得一条弦技");
    assert!(g.counter("juexiang.protect.1") > 0, "获得花色保护");
    // 梅花牌对其无效。
    add_suit(&mut g, "c", "sha", Zone::Hand(0), Suit::Club, 7);
    assert!(b8::card_ineffective_for(&mut g, 0, 1, "c"));
    // 到其下回合开始失效。
    b8::on_turn_start(&mut g, 1);
    assert!(!b8::card_ineffective_for(&mut g, 0, 1, "c"));
}

#[test]
fn xian_skills_execute_branches() {
    let mut g = game(2, &[]);
    g.seats[0].granted = vec!["jixian".to_string()];
    g.seats[1].hp = 3;
    add(&mut g, "eq", "bagua_zhen", Zone::Equip(1, Slot::Armor));
    let step = run_steps(&mut g, "jixian", 0, data(&[("by", 1.into())]), &["use", "opt2", "card:eq"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.seats[1].hp, 4, "柔弦/和弦式的回复分支");
}

// ---------------------------------------------------------------------------
// 谏征 / 专对 / 天辩
// ---------------------------------------------------------------------------

#[test]
fn jianzheng_cancels_targets_and_becomes_target_if_not_black() {
    let mut g = game(3, &["jianzheng"]);
    g.turn = 1;
    g.phase = Phase::Play;
    add_suit(&mut g, "s", "sha", Zone::Hand(1), Suit::Heart, 7);
    add(&mut g, "h", "tao", Zone::Hand(0));
    let step = run_steps(
        &mut g,
        "jianzheng",
        0,
        data(&[("step", "sha_targeted".into()), ("user", 1.into()), ("card", "s".into())]),
        &["use", "card:h"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(b8::take_sha_targets(&mut g, "s"), Some(vec![0]), "非黑【杀】→ 秦宓成为目标");
}

#[test]
fn tianbian_deck_option_and_heart_as_king() {
    let mut g = game(2, &["tianbian"]);
    add(&mut g, "d", "shan", Zone::Draw);
    g.deck.push("d".into());
    add_suit(&mut g, "h", "tao", Zone::Hand(0), Suit::Heart, 2);
    assert!(b8::pindian_deck_option(&g, 0));
    assert_eq!(b8::pindian_rank(&g, 0, "h"), 13, "红桃视为 K");
    // 反例：无天辩时点数按牌面。
    let mut g2 = game(2, &[]);
    add_suit(&mut g2, "h", "tao", Zone::Hand(0), Suit::Heart, 2);
    assert_eq!(b8::pindian_rank(&g2, 0, "h"), 2);
}

#[test]
fn zhuandui_winning_as_user_forbids_response() {
    let mut g = game(2, &["zhuandui"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_suit(&mut g, "s", "sha", Zone::Hand(0), Suit::Spade, 13);
    add_suit(&mut g, "t", "tao", Zone::Hand(1), Suit::Heart, 2);
    resolve::push_ask(
        &mut g,
        "zhuandui",
        Stage::Choose,
        0,
        data(&[("step", "sha_targeted".into()), ("role", "user".into()), ("user", 0.into()), ("targets", serde_json::json!([1])), ("card", "s".into())]),
    );
    resolve::advance(&mut g).expect("询问");
    submit(&mut g, 0, "use").expect("发动专对");
    // 拼点：双方各出一张（K vs 2）。
    submit(&mut g, 0, "choose:s").expect("拼点出牌");
    submit(&mut g, 1, "choose:t").expect("拼点出牌");
    assert!(g.use_opts("s").no_response, "赢→目标不能响应");
}

// ---------------------------------------------------------------------------
// 复难 / 诫训 / 辟撰 / 通博
// ---------------------------------------------------------------------------

#[test]
fn funan_swaps_cards_and_freezes_used_card() {
    let mut g = game(2, &["funan"]);
    add(&mut g, "used", "sha", Zone::Discard);
    add(&mut g, "resp", "shan", Zone::Discard);
    g.set_mark("responds.resp", "used");
    g.set_mark("used_by.used", "0");
    let step = run_steps(
        &mut g,
        "funan",
        0,
        data(&[("card", "resp".into()), ("victim", 1.into())]),
        &["use"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.hand(0).contains(&"resp".to_string()), "获得响应牌");
    assert!(g.hand(1).contains(&"used".to_string()), "响应者获得使用牌");
    assert!(g.is_frozen(1, "used"), "本回合不能使用/打出该牌");
}

#[test]
fn funan_plus_only_gains_response_card() {
    let mut g = game(2, &["funan"]);
    add(&mut g, "used", "sha", Zone::Discard);
    add(&mut g, "resp", "shan", Zone::Discard);
    g.set_mark("responds.resp", "used");
    g.set_mark("used_by.used", "0");
    g.bump("funan.plus.0");
    let step = run_steps(
        &mut g,
        "funan",
        0,
        data(&[("card", "resp".into()), ("victim", 1.into())]),
        &["use"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.hand(0).contains(&"resp".to_string()));
    assert!(g.cards.get("used") == Some(&Zone::Discard), "对方不再获得使用牌");
}

#[test]
fn jiexun_draws_diamonds_and_upgrades_funan_when_target_empties() {
    let mut g = game(2, &["jiexun", "funan"]);
    g.turn = 0;
    g.phase = Phase::End;
    add_suit(&mut g, "d", "qinggang_jian", Zone::Equip(1, Slot::Weapon), Suit::Club, 6);
    add_deck(&mut g, 5);
    let step = run_steps(&mut g, "jiexun", 0, data(&[]), &["t:1", "card:d"]);
    let step = drain(&mut g, "jiexun", 0, step);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.counter("jiexun.used.0"), 1);
    assert!(!g.seats[0].skills.iter().any(|s| s == "jiexun"), "目标弃光 → 失去诫训");
    assert!(g.counter("funan.plus.0") > 0, "复难升级");
}

#[test]
fn bizhuan_puts_spade_top_card_as_book_and_raises_hand_limit() {
    let mut g = game(2, &["bizhuan"]);
    add_suit(&mut g, "s", "sha", Zone::Hand(0), Suit::Spade, 7);
    add(&mut g, "top", "tao", Zone::Draw);
    g.deck.push("top".into());
    let step = run_steps(&mut g, "bizhuan", 0, data(&[("card", "s".into()), ("by", 0.into())]), &["use"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.cards.get("top") == Some(&Zone::Mark(0, "shu".to_string())), "「书」移出游戏");
    assert_eq!(g.hand_limit(0), 5, "每张书手牌上限 +1");
    // 反例：红色牌不触发。
    let mut g2 = game(2, &["bizhuan"]);
    add_suit(&mut g2, "h", "sha", Zone::Hand(0), Suit::Heart, 7);
    assert!(matches!(call(&mut g2, "bizhuan", 0, data(&[("card", "h".into())])), Step::Done));
}

#[test]
fn tongbo_replaces_books_and_gives_them_away_with_four_suits() {
    let mut g = game(2, &["tongbo"]);
    g.turn = 0;
    g.phase = Phase::Draw;
    // 四张「书」覆盖四花色。
    for (i, suit) in [Suit::Spade, Suit::Heart, Suit::Club, Suit::Diamond].iter().enumerate() {
        let id = format!("b{i}");
        add_suit(&mut g, &id, "tao", Zone::Mark(0, "shu".to_string()), *suit, 3);
    }
    add_suit(&mut g, "h", "sha", Zone::Hand(0), Suit::Spade, 9);
    let step = run_steps(
        &mut g,
        "tongbo",
        0,
        data(&[]),
        &["use", "card:h", "card:b0", "pass", "t:1"],
    );
    let step = drain(&mut g, "tongbo", 0, step);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(1), 4, "所有「书」交给其");
}
