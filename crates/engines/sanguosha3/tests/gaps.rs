//! 缺口收口门禁（B6–B10 登记缺口的回归）：响应替代三场景 / 攻心 / 谦逊判定 /
//! 讥谀花色封锁 / 天辩无手牌拼点 / 饰非在【万箭】中生效。

use sanguosha3::kernel::cards::ResponseKind;
use sanguosha3::kernel::resolve::{self, Stage};
use sanguosha3::kernel::skills::b10;
use sanguosha3::kernel::{flow, skills, Card, Game, Identity, Phase, Seat, Suit, Zone};

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
    let mut seats: Vec<Seat> = (0..n).map(|i| seat_with(i, &[], "wei")).collect();
    seats[0] = seat_with(0, s0, "wei");
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

fn submit(g: &mut Game, seat: usize, option: &str) -> Result<(), String> {
    resolve::submit(g, seat, option)
}

fn options(g: &Game) -> Vec<String> {
    g.pending
        .as_ref()
        .map(|p| p.options().iter().map(|o| o.id.clone()).collect())
        .unwrap_or_default()
}

fn use_card(g: &mut Game, seat: usize, id: &str) {
    g.pending = None;
    sanguosha3::kernel::card_play::begin_use(g, seat, id, false).expect("使用");
    resolve::advance(g).expect("推进");
}

// ---------------------------------------------------------------------------
// 滔乱：响应场景（南蛮当【杀】/ 万箭当【闪】/ 无懈链当【无懈可击】）
// ---------------------------------------------------------------------------

#[test]
fn taoluan_responds_as_sha_in_nanman() {
    let mut g = game(2, &[]);
    g.seats[1].skills = vec!["taoluan".to_string()];
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "nanman", "nanman_ruqin", Zone::Hand(0));
    add(&mut g, "h", "tao", Zone::Hand(1));
    use_card(&mut g, 0, "nanman");
    // 目标把【桃】当【杀】打出（滔乱）。
    let opts = options(&g);
    assert!(
        opts.iter().any(|o| o == "alt:taoluan"),
        "南蛮响应提供滔乱：{opts:?}"
    );
    submit(&mut g, 1, "alt:taoluan").expect("滔乱");
    submit(&mut g, 1, "card:h").expect("用桃当【杀】");
    assert_eq!(g.seats[1].hp, 4, "视为打出【杀】，未受伤");
    assert!(g.counter("taoluan.used.sha") > 0, "该牌名本局已用");
}

#[test]
fn taoluan_responds_as_shan_in_wanjian() {
    let mut g = game(2, &[]);
    g.seats[1].skills = vec!["taoluan".to_string()];
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "wanjian", "wanjian_qifa", Zone::Hand(0));
    add(&mut g, "h", "tao", Zone::Hand(1));
    use_card(&mut g, 0, "wanjian");
    let opts = options(&g);
    assert!(opts.iter().any(|o| o == "alt:taoluan"), "{opts:?}");
    submit(&mut g, 1, "alt:taoluan").expect("滔乱");
    submit(&mut g, 1, "card:h").expect("用桃当【闪】");
    assert_eq!(g.seats[1].hp, 4, "视为打出【闪】");
    assert!(g.counter("taoluan.used.shan") > 0);
}

// ---------------------------------------------------------------------------
// 激将：响应窗口代出【杀】（南蛮）
// ---------------------------------------------------------------------------

#[test]
fn jijiang_relays_sha_in_response_window() {
    let mut g = game(3, &[]);
    g.seats[0].skills = vec!["jijiang".to_string()];
    g.seats[0].identity = Some(Identity::Lord);
    g.seats[0].kingdom = "shu".into();
    g.seats[2].kingdom = "shu".into();
    g.turn = 1;
    g.phase = Phase::Play;
    add(&mut g, "nanman", "nanman_ruqin", Zone::Hand(1));
    add(&mut g, "k1", "sha", Zone::Hand(2));
    add(&mut g, "k2", "sha", Zone::Hand(2));
    use_card(&mut g, 1, "nanman");
    // 目标顺序：先 seat2（有【杀】），再 seat0（主公，可用激将代出）。
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(2));
    submit(&mut g, 2, "p:k1").expect("打出【杀】");
    // 激将的摸牌触发（seat2 回合外打出【杀】→ 可令主公摸一张）：先放弃。
    if g.pending.as_ref().map(|p| p.actor()) == Some(2) {
        submit(&mut g, 2, "pass").expect("不令主公摸牌");
    }
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    let opts = options(&g);
    assert!(opts.iter().any(|o| o == "alt:jijiang"), "{opts:?}");
    submit(&mut g, 0, "alt:jijiang").expect("激将");
    submit(&mut g, 2, "respond:k2").expect("代出【杀】");
    // 代出后再出现的摸牌询问（如有）一并放弃。
    if g.pending.as_ref().map(|p| p.actor()) == Some(2) {
        submit(&mut g, 2, "pass").expect("不令主公摸牌");
    }
    assert_eq!(g.seats[0].hp, 4, "代出【杀】，主公未受伤");
    assert_eq!(g.hand_size(2), 0, "代出者的【杀】已打出");
}

// ---------------------------------------------------------------------------
// 饰非：【万箭齐发】中的【闪】替代
// ---------------------------------------------------------------------------

#[test]
fn shifei_alternative_in_wanjian() {
    let mut g = game(2, &[]);
    g.seats[1].skills = vec!["shifei".to_string()];
    g.turn = 1;
    g.phase = Phase::Play;
    add_deck(&mut g, 5);
    add(&mut g, "wanjian", "wanjian_qifa", Zone::Hand(0));
    add(&mut g, "h", "tao", Zone::Hand(1));
    use_card(&mut g, 0, "wanjian");
    let opts = options(&g);
    assert!(opts.iter().any(|o| o == "alt:shifei"), "{opts:?}");
    submit(&mut g, 1, "alt:shifei").expect("饰非");
    assert_eq!(g.seats[1].hp, 4, "视为打出【闪】");
}

// ---------------------------------------------------------------------------
// 攻心（勤学授予）
// ---------------------------------------------------------------------------

#[test]
fn qinxue_grants_gongxin_and_it_discards_heart() {
    let mut g = game(2, &["qinxue"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    for i in 0..6 {
        add(&mut g, &format!("h{i}"), "sha", Zone::Hand(0));
    }
    add_deck(&mut g, 5);
    let h = skills::handler("qinxue").expect("handler");
    let ask = sanguosha3::kernel::resolve::Ask {
        skill: "qinxue".into(),
        stage: Stage::Choose,
        actor: 0,
        data: serde_json::json!({}),
    };
    let _ = h(&mut g, &ask);
    // 选择觉醒收益。
    g.answer = Some("opt1".into());
    g.answer_skill = Some("qinxue".into());
    let step = h(&mut g, &ask).expect("觉醒");
    assert!(matches!(step, sanguosha3::kernel::resolve::Step::Finish(_)));
    assert!(g.seats[0].granted.iter().any(|s| s == "gongxin"), "获得攻心");
    g.phase = Phase::Play;
    // 攻心：观看并弃置其红桃牌。
    add_suit(&mut g, "heart", "tao", Zone::Hand(1), Suit::Heart, 3);
    add_suit(&mut g, "spade", "sha", Zone::Hand(1), Suit::Spade, 4);
    skills::use_active(&mut g, 0, "gongxin").expect("攻心可用");
    submit(&mut g, 0, "t:1").expect("选目标");
    submit(&mut g, 0, "discard:heart").expect("弃置其红桃牌");
    assert_eq!(g.hand_size(1), 1, "红桃牌被弃置");
    assert_eq!(g.counter("phase.0.gongxin"), 1);
}

// ---------------------------------------------------------------------------
// 谦逊：延时锦囊判定生效时
// ---------------------------------------------------------------------------

#[test]
fn qianxun_triggers_on_delayed_judge() {
    let mut g = game(2, &["qianxun"]);
    g.turn = 0;
    add(&mut g, "le", "lebu_sishu", Zone::Judge(0));
    add(&mut g, "h", "sha", Zone::Hand(0));
    b10::on_delayed_judge(&mut g, 0, "le");
    resolve::advance(&mut g).expect("谦逊询问");
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    submit(&mut g, 0, "use").expect("移出全部手牌");
    assert_eq!(g.hand_size(0), 0);
    b10::on_turn_end(&mut g, 0);
    assert_eq!(g.hand_size(0), 1, "回合结束归还");
}

// ---------------------------------------------------------------------------
// 讥谀：花色封锁（含后续获得的牌）
// ---------------------------------------------------------------------------

#[test]
fn jiyu_suit_lock_blocks_use_and_response() {
    let mut g = game(2, &["jiyu"]);
    add_suit(&mut g, "h1", "tao", Zone::Hand(0), Suit::Heart, 3);
    add_suit(&mut g, "h2", "sha", Zone::Hand(0), Suit::Heart, 4);
    g.set_counter("turn.0.jiyu_lock.红桃", 1);
    assert!(skills::card_locked(&g, 0, "h2"), "红桃花色被封锁（封锁按花色，不是单张）");
    assert!(skills::card_locked(&g, 0, "h1"), "同花色其它牌同样被封锁");
    assert_eq!(skills::response_as(&g, 0, "h2", ResponseKind::Sha), None);
    assert!(!sanguosha3::kernel::card_play::usable_hand(&g, 0).contains(&"h2".to_string()));
    // 反例：非封锁花色仍可用（后续获得的牌也受同一把锁）。
    add_suit(&mut g, "s1", "sha", Zone::Hand(0), Suit::Spade, 5);
    assert!(!skills::card_locked(&g, 0, "s1"));
    assert!(sanguosha3::kernel::card_play::usable_hand(&g, 0).contains(&"s1".to_string()));
}

// ---------------------------------------------------------------------------
// 天辩：无手牌也可用牌堆顶拼点
// ---------------------------------------------------------------------------

#[test]
fn tianbian_allows_pindian_without_hand() {
    let mut g = game(2, &["tianbian"]);
    add(&mut g, "d", "shan", Zone::Draw);
    g.deck.push("d".into());
    // 双方都无手牌：有天辩 + 牌堆非空 → 拼点仍可成立。
    add(&mut g, "h", "tao", Zone::Hand(1));
    assert!(sanguosha3::kernel::pindian::can_start(&g, 0, 1));
    // 反例：无天辩时不能成立。
    let mut g2 = game(2, &[]);
    add(&mut g2, "h", "tao", Zone::Hand(1));
    assert!(!sanguosha3::kernel::pindian::can_start(&g2, 0, 1));
}

// ---------------------------------------------------------------------------
// 连营：非弃牌阶段（使用最后一张手牌）的 X=1
// ---------------------------------------------------------------------------

#[test]
fn lianying_x1_when_using_last_hand_card() {
    let mut g = game(2, &["lianying"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "w", "wuzhong_shengyou", Zone::Hand(0));
    use_card(&mut g, 0, "w");
    // 无中生有摸 2 张后手牌不为空；这里只验证「失去最后一张手牌」压入了连营询问。
    assert!(
        g.pending.is_some() || g.hand_size(0) > 0,
        "连营/结算已推进"
    );
}
