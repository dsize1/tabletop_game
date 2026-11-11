//! M1 卡牌结算内核验收测试（`memory_palace/plan/next-engine-replacement.md` §M1）。
//!
//! 覆盖：基本牌（杀/闪/桃/酒）→ 装备数值链（八卦/仁王盾/藤甲/白银狮子/青釭剑/坐骑/武器范围）
//! → 普通锦囊（决斗/过河/顺手/无中生有/南蛮/万箭/桃园/五谷/火攻/借刀/铁索/无懈链）
//! → 延时锦囊（乐/兵/闪电 LIFO 判定）→ 牌区不变量 / 单动作耗时基线 / 卡牌门禁。
//!
//! 所有断言都读**属性**（`cards::def`）或状态（牌区/体力/事件），不依赖牌名分支。

use std::time::Instant;

use sanguosha2::cards::{self, ArmorKind, DamageKind, Effect, ResponseKind};
use sanguosha2::kernel::pending::Pending;
use sanguosha2::kernel::resolve::{begin_phase, submit};
use sanguosha2::kernel::{CardId, Game, Phase, Seat, Suit, Zone};
use sanguosha2::presentation::POutcome;

// ---------------------------------------------------------------------------
// 夹具
// ---------------------------------------------------------------------------

fn new_game(n: usize) -> Game {
    let seats: Vec<Seat> = (0..n).map(|i| Seat::new(&format!("p{i}"), "guest", &[], 4)).collect();
    let mut g = Game::new(seats, cards::deck());
    begin_phase(&mut g, 0, Phase::Play).expect("进入 p0 出牌阶段");
    g
}

/// 从牌堆取一张指定牌名的牌交到某人手里（返回 id）。
fn give(g: &mut Game, seat: usize, name: &str) -> CardId {
    let id = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| g.name_of(id) == name)
        .unwrap_or_else(|| panic!("牌堆里没有【{name}】"));
    g.to_hand(&id, seat).expect("给牌");
    id
}

/// 从牌堆取一张「牌名 + 花色」都匹配的牌。
fn give_suit(g: &mut Game, seat: usize, name: &str, suit: Suit) -> CardId {
    let id = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| {
            g.name_of(id) == name && g.cards.get(id).map(|c| c.suit) == Some(suit)
        })
        .unwrap_or_else(|| panic!("牌堆里没有 {suit:?} 的【{name}】"));
    g.to_hand(&id, seat).expect("给牌");
    id
}

/// 把满足条件的牌置于牌堆顶。
fn put_top_where(g: &mut Game, pred: impl Fn(&sanguosha2::kernel::Card) -> bool) -> CardId {
    let id = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| g.cards.get(id).map(&pred).unwrap_or(false))
        .expect("没有满足条件的牌");
    g.put_on_top(&id).expect("置于牌堆顶");
    id
}

fn equip(g: &mut Game, seat: usize, name: &str) -> CardId {
    let id = give(g, seat, name);
    let slot = g.def_of(&id).slot.expect("装备牌应有槽位");
    g.to_equip(&id, seat, slot).expect("装备");
    id
}

fn pend(g: &Game) -> &Pending {
    g.pending.as_ref().expect("应有 pending")
}

fn options(g: &Game) -> Vec<String> {
    pend(g).options().iter().map(|o| o.id.clone()).collect()
}

fn opt_with(g: &Game, prefix: &str) -> String {
    options(g)
        .into_iter()
        .find(|o| o.starts_with(prefix))
        .unwrap_or_else(|| panic!("缺少选项 {prefix}：{:?}", options(g)))
}

fn answer(g: &mut Game, actor: usize, action: &str) {
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
    invariant(g);
}

fn invariant(g: &Game) {
    g.check_conservation().expect("牌区守恒");
    g.check_no_orphan_frames().expect("无帧遗留");
}

/// 结算一张牌；有 pending 时由 `script` 逐次作答。
fn play(g: &mut Game, seat: usize, card: &CardId, targets: Vec<usize>, script: &mut dyn FnMut(&mut Game)) {
    g.resolve_use(seat, card, targets)
        .unwrap_or_else(|e| panic!("使用【{}】失败：{e}", g.name_of(card)));
    invariant(g);
    script(g);
    invariant(g);
    assert!(g.stack.is_empty(), "结算栈应已清空：{:?} / pending={:?}", g.stack, g.pending);
}

/// 无 pending 的简单牌：直接结算。
fn play_simple(g: &mut Game, seat: usize, card: &CardId, targets: Vec<usize>) {
    play(g, seat, card, targets, &mut |g| {
        assert!(g.pending.is_none(), "不应询问：{:?}", g.pending);
    });
}

// ---------------------------------------------------------------------------
// 牌堆 / 数据门禁
// ---------------------------------------------------------------------------

#[test]
fn deck_is_160_and_conserved() {
    let g = Game::new(vec![Seat::new("p0", "guest", &[], 4)], cards::deck());
    assert_eq!(cards::DECK_SIZE, 160);
    assert_eq!(g.cards.len(), 160);
    g.check_conservation().unwrap();
    g.check_no_orphan_frames().unwrap();
}

#[test]
fn every_deck_card_has_definition() {
    for c in cards::deck() {
        let def = cards::def(&c.name);
        assert!(def.known, "未知牌名：{}", c.name);
        // 每张牌要么是装备，要么有明确效果（禁止「无效果锦囊」混进牌堆）。
        assert!(
            def.kind == cards::CardKind::Equip || def.effect != Effect::None,
            "牌 {} 缺少效果",
            c.name
        );
    }
}

#[test]
fn deck_attributes_follow_generated_data() {
    assert_eq!(cards::def("杀").effect, Effect::Sha);
    assert_eq!(cards::def("火杀").damage_kind, DamageKind::Fire);
    assert_eq!(cards::def("雷杀").damage_kind, DamageKind::Thunder);
    assert_eq!(cards::def("藤甲").armor, ArmorKind::Tengjia);
    assert_eq!(cards::def("白银狮子").armor, ArmorKind::Baiyin);
    assert_eq!(cards::def("万箭齐发").tags.respond, Some(ResponseKind::Shan));
    assert_eq!(cards::def("南蛮入侵").tags.respond, Some(ResponseKind::Sha));
}

// ---------------------------------------------------------------------------
// 杀：命中 / 响应 / 距离 / 次数 / 酒 / 属性 / 连环
// ---------------------------------------------------------------------------

#[test]
fn sha_hits_target_without_shan() {
    let mut g = new_game(3);
    let sha = give(&mut g, 0, "杀");
    let hp = g.seats[1].hp;
    play_simple(&mut g, 0, &sha, vec![1]);
    assert_eq!(g.seats[1].hp, hp - 1);
    assert_eq!(g.zone_of(&sha), Some(&Zone::Discard));
}

#[test]
fn sha_asks_for_shan_and_shan_negates() {
    let mut g = new_game(3);
    let sha = give(&mut g, 0, "杀");
    give(&mut g, 1, "闪");
    let hp = g.seats[1].hp;
    play(&mut g, 0, &sha, vec![1], &mut |g| {
        assert_eq!(pend(g).actor(), 1, "应由目标响应");
        assert_eq!(pend(g).skill(), Some("__card"));
        let shan = opt_with(g, "respond:");
        answer(g, 1, &shan);
    });
    assert_eq!(g.seats[1].hp, hp, "闪抵消杀");
}

#[test]
fn sha_target_can_decline_response() {
    let mut g = new_game(3);
    let sha = give(&mut g, 0, "杀");
    give(&mut g, 1, "闪");
    let hp = g.seats[1].hp;
    play(&mut g, 0, &sha, vec![1], &mut |g| {
        let escape = pend(g).escape_id().to_string();
        answer(g, 1, &escape);
    });
    assert_eq!(g.seats[1].hp, hp - 1);
}

#[test]
fn sha_out_of_range_is_rejected() {
    let mut g = new_game(4);
    let sha = give(&mut g, 0, "杀");
    // 4 人局 0 与 2 距离 2 > 攻击范围 1。
    let err = g.resolve_use(0, &sha, vec![2]).unwrap_err();
    assert!(err.contains("攻击范围"), "{err}");
    assert_eq!(g.zone_of(&sha), Some(&Zone::Hand(0)), "牌不离开手牌");
    assert!(g.pending.is_none());
}

#[test]
fn sha_limit_is_one_per_turn() {
    let mut g = new_game(3);
    let a = give(&mut g, 0, "杀");
    let b = give(&mut g, 0, "杀");
    play_simple(&mut g, 0, &a, vec![1]);
    let err = g.resolve_use(0, &b, vec![1]).unwrap_err();
    assert!(err.contains("次数"), "{err}");
}

#[test]
fn sha_cannot_target_self() {
    let mut g = new_game(3);
    let sha = give(&mut g, 0, "杀");
    assert!(g.resolve_use(0, &sha, vec![0]).is_err());
}

#[test]
fn jiu_adds_one_damage_to_next_sha() {
    let mut g = new_game(3);
    let jiu = give(&mut g, 0, "酒");
    let sha = give(&mut g, 0, "杀");
    play_simple(&mut g, 0, &jiu, vec![]);
    assert_eq!(g.seats[0].hp, 4, "酒不回复体力");
    let hp = g.seats[1].hp;
    play_simple(&mut g, 0, &sha, vec![1]);
    assert_eq!(g.seats[1].hp, hp - 2, "酒加成 +1");
}

#[test]
fn jiu_is_limited_once_per_turn() {
    let mut g = new_game(3);
    let a = give(&mut g, 0, "酒");
    let b = give(&mut g, 0, "酒");
    play_simple(&mut g, 0, &a, vec![]);
    assert!(g.resolve_use(0, &b, vec![]).is_err());
}

#[test]
fn jiu_bonus_expires_after_one_sha() {
    let mut g = new_game(3);
    let jiu = give(&mut g, 0, "酒");
    let sha1 = give(&mut g, 0, "杀");
    let sha2 = give(&mut g, 0, "杀");
    play_simple(&mut g, 0, &jiu, vec![]);
    // 覆盖杀次数限制以验证第二张杀回到 1 点伤害。
    g.counters.insert("turn.jueyan_sha_bonus".into(), 1);
    let hp = g.seats[1].hp;
    play_simple(&mut g, 0, &sha1, vec![1]);
    assert_eq!(g.seats[1].hp, hp - 2);
    let hp = g.seats[1].hp;
    play_simple(&mut g, 0, &sha2, vec![1]);
    assert_eq!(g.seats[1].hp, hp - 1, "酒加成只作用于下一张杀");
}

#[test]
fn thunder_sha_propagates_through_chain() {
    let mut g = new_game(3);
    let sha = give(&mut g, 0, "雷杀");
    g.seats[1].chained = true;
    g.seats[2].chained = true;
    let (h1, h2) = (g.seats[1].hp, g.seats[2].hp);
    play_simple(&mut g, 0, &sha, vec![1]);
    assert_eq!(g.seats[1].hp, h1 - 1, "目标受到雷伤");
    assert_eq!(g.seats[2].hp, h2 - 1, "连环传导到 p2");
    assert!(!g.seats[1].chained && !g.seats[2].chained, "属性伤害后解除连环");
}

#[test]
fn fire_sha_against_tengjia_deals_two() {
    let mut g = new_game(3);
    let sha = give(&mut g, 0, "火杀");
    equip(&mut g, 1, "藤甲");
    let hp = g.seats[1].hp;
    play_simple(&mut g, 0, &sha, vec![1]);
    assert_eq!(g.seats[1].hp, hp - 2, "藤甲：火焰伤害 +1");
}

// ---------------------------------------------------------------------------
// 防具数值链
// ---------------------------------------------------------------------------

#[test]
fn renwang_blocks_black_sha() {
    let mut g = new_game(3);
    let sha = give_suit(&mut g, 0, "杀", Suit::Spade);
    equip(&mut g, 1, "仁王盾");
    let hp = g.seats[1].hp;
    play_simple(&mut g, 0, &sha, vec![1]);
    assert_eq!(g.seats[1].hp, hp, "仁王盾：黑色杀无效");
}

#[test]
fn renwang_does_not_block_red_sha() {
    let mut g = new_game(3);
    let sha = give_suit(&mut g, 0, "杀", Suit::Heart);
    equip(&mut g, 1, "仁王盾");
    let hp = g.seats[1].hp;
    play_simple(&mut g, 0, &sha, vec![1]);
    assert_eq!(g.seats[1].hp, hp - 1);
}

#[test]
fn qinggang_ignores_armor() {
    let mut g = new_game(3);
    equip(&mut g, 0, "青釭剑");
    equip(&mut g, 1, "仁王盾");
    let sha = give_suit(&mut g, 0, "杀", Suit::Spade);
    let hp = g.seats[1].hp;
    play_simple(&mut g, 0, &sha, vec![1]);
    assert_eq!(g.seats[1].hp, hp - 1, "青釭剑无视防具");
}

#[test]
fn tengjia_blocks_normal_sha_and_aoe() {
    let mut g = new_game(3);
    equip(&mut g, 1, "藤甲");
    let sha = give_suit(&mut g, 0, "杀", Suit::Heart);
    let hp = g.seats[1].hp;
    play_simple(&mut g, 0, &sha, vec![1]);
    assert_eq!(g.seats[1].hp, hp, "藤甲：普通杀无效");

    let nanman = give(&mut g, 0, "南蛮入侵");
    give(&mut g, 2, "杀"); // p2 有杀可响应
    let hp = g.seats[1].hp;
    play(&mut g, 0, &nanman, vec![], &mut |g| {
        // p2 有【杀】→ 询问；p1 被藤甲挡下（不询问）。
        assert_eq!(pend(g).actor(), 2, "只询问未装备藤甲的目标");
        answer(g, 2, "pass");
    });
    assert_eq!(g.seats[1].hp, hp, "藤甲：南蛮无效");
}

#[test]
fn baiyin_caps_damage_and_heals_when_lost() {
    let mut g = new_game(3);
    let jiu = give(&mut g, 0, "酒");
    let sha = give(&mut g, 0, "杀");
    equip(&mut g, 1, "白银狮子");
    play_simple(&mut g, 0, &jiu, vec![]);
    let hp = g.seats[1].hp;
    play_simple(&mut g, 0, &sha, vec![1]);
    assert_eq!(g.seats[1].hp, hp - 1, "白银狮子：伤害 >1 改为 1");

    // 失去白银狮子 → 回复 1（换装走真实结算入口，需要 p1 的回合）。
    g.seats[1].hp = 2;
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    let bagua = give(&mut g, 1, "八卦阵");
    play_simple(&mut g, 1, &bagua, vec![]);
    assert_eq!(g.seats[1].hp, 3, "失去白银狮子回复 1 点");
}

#[test]
fn bagua_red_judge_counts_as_shan() {
    let mut g = new_game(3);
    equip(&mut g, 1, "八卦阵");
    let sha = give(&mut g, 0, "杀");
    let red = put_top_where(&mut g, |c| c.suit.is_red());
    let hp = g.seats[1].hp;
    play(&mut g, 0, &sha, vec![1], &mut |g| {
        assert!(options(g).contains(&"armor:bagua".to_string()));
        answer(g, 1, "armor:bagua");
    });
    assert_eq!(g.seats[1].hp, hp, "八卦红色 → 视为闪");
    assert_eq!(g.zone_of(&red), Some(&Zone::Discard), "判定牌进弃牌堆");
}

#[test]
fn bagua_black_judge_fails() {
    let mut g = new_game(3);
    equip(&mut g, 1, "八卦阵");
    let sha = give(&mut g, 0, "杀");
    put_top_where(&mut g, |c| !c.suit.is_red());
    let hp = g.seats[1].hp;
    play(&mut g, 0, &sha, vec![1], &mut |g| {
        answer(g, 1, "armor:bagua");
    });
    assert_eq!(g.seats[1].hp, hp - 1, "八卦黑色 → 判定失败");
}

#[test]
fn baiyin_is_applied_after_skill_and_card_bonus() {
    // 装备数值链顺序：技能/酒加成 → 藤甲(+1) → 白银狮子（最后：>1 改为 1）。
    let mut g = new_game(3);
    let jiu = give(&mut g, 0, "酒");
    let sha = give_suit(&mut g, 0, "火杀", Suit::Heart);
    equip(&mut g, 1, "白银狮子");
    let hp = g.seats[1].hp;
    play_simple(&mut g, 0, &jiu, vec![]);
    play_simple(&mut g, 0, &sha, vec![1]);
    assert_eq!(g.seats[1].hp, hp - 1, "酒+火杀=2 → 白银狮子改为 1");
}

// ---------------------------------------------------------------------------
// 装备：槽位 / 范围 / 坐骑 / 废除栏
// ---------------------------------------------------------------------------

#[test]
fn weapon_range_extends_attack_reach() {
    let mut g = new_game(4);
    let sha = give(&mut g, 0, "杀");
    assert!(g.resolve_use(0, &sha, vec![2]).is_err(), "默认范围 1 打不到");
    let bow = give(&mut g, 0, "麒麟弓");
    play_simple(&mut g, 0, &bow, vec![]);
    assert_eq!(g.attack_range(0), 5);
    play_simple(&mut g, 0, &sha, vec![2]);
    assert_eq!(g.zone_of(&sha), Some(&Zone::Discard));
}

#[test]
fn offense_horse_reduces_distance() {
    let mut g = new_game(4);
    assert_eq!(g.distance(0, 2), 2);
    let horse = give(&mut g, 0, "赤兔");
    play_simple(&mut g, 0, &horse, vec![]);
    assert_eq!(g.distance(0, 2), 1, "进攻马 -1 距离");
    let sha = give(&mut g, 0, "杀");
    play_simple(&mut g, 0, &sha, vec![2]);
}

#[test]
fn defense_horse_increases_incoming_distance() {
    let mut g = new_game(4);
    equip(&mut g, 2, "的卢");
    assert_eq!(g.distance(0, 2), 3, "防御马：别人到 p2 的距离 +1");
}

#[test]
fn equip_replaces_same_slot() {
    let mut g = new_game(3);
    let a = give(&mut g, 0, "青釭剑");
    play_simple(&mut g, 0, &a, vec![]);
    let b = give(&mut g, 0, "青龙偃月刀");
    play_simple(&mut g, 0, &b, vec![]);
    assert_eq!(g.zone_of(&a), Some(&Zone::Discard), "旧武器进弃牌堆");
    assert_eq!(g.equip_in(0, sanguosha2::kernel::Slot::Weapon), Some(b));
}

#[test]
fn abolished_slot_cannot_equip() {
    let mut g = new_game(3);
    g.abolish_slot(0, sanguosha2::kernel::Slot::Armor).unwrap();
    let armor = give(&mut g, 0, "八卦阵");
    let err = g.resolve_use(0, &armor, vec![]).unwrap_err();
    assert!(err.contains("废除"), "{err}");
}

// ---------------------------------------------------------------------------
// 桃 / 闪 / 无懈 的基本约束
// ---------------------------------------------------------------------------

#[test]
fn tao_heals_self_only() {
    let mut g = new_game(3);
    g.seats[0].hp = 2;
    let tao = give(&mut g, 0, "桃");
    play_simple(&mut g, 0, &tao, vec![]);
    assert_eq!(g.seats[0].hp, 3);
}

#[test]
fn tao_rejected_at_full_hp_or_on_others() {
    let mut g = new_game(3);
    let tao = give(&mut g, 0, "桃");
    assert!(g.resolve_use(0, &tao, vec![]).is_err(), "满血不能用桃");
    g.seats[0].hp = 2;
    assert!(g.resolve_use(0, &tao, vec![1]).is_err(), "桃只能对自己");
}

#[test]
fn shan_and_wuxie_cannot_be_used_proactively() {
    let mut g = new_game(3);
    let shan = give(&mut g, 0, "闪");
    let wuxie = give(&mut g, 0, "无懈可击");
    assert!(g.resolve_use(0, &shan, vec![]).is_err());
    assert!(g.resolve_use(0, &wuxie, vec![]).is_err());
}

#[test]
fn tao_heal_is_capped_at_max_hp() {
    let mut g = new_game(3);
    g.seats[0].hp = 3;
    let tao = give(&mut g, 0, "桃");
    play_simple(&mut g, 0, &tao, vec![]);
    assert_eq!(g.seats[0].hp, 4, "不能超过体力上限");
}

// ---------------------------------------------------------------------------
// 单动作耗时基线（D4）
// ---------------------------------------------------------------------------

#[test]
fn single_card_action_perf_baseline() {
    let mut worst = 0f64;
    let mut total = 0f64;
    let n = 60;
    for _ in 0..n {
        let mut g = new_game(3);
        let sha = give(&mut g, 0, "杀");
        let t = Instant::now();
        play_simple(&mut g, 0, &sha, vec![1]);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        worst = worst.max(ms);
        total += ms;
    }
    let p95 = worst; // n 小，取最坏值作为保守上界
    println!(
        "单动作基线：{} 次 杀 结算，total={total:.2}ms avg={:.2}ms worst(p95)={p95:.2}ms",
        n,
        total / n as f64
    );
    assert!(p95 < 200.0, "单动作 p95 超基线：{p95:.2}ms");
    assert!(total < 5000.0, "60 次结算总耗时过长：{total:.2}ms");
}

// 引用避免 `DamageKind`/`POutcome` 未使用（后续块使用；保留显式断言）。
#[test]
fn damage_kind_and_outcome_are_typed() {
    assert_eq!(cards::def("火杀").damage_kind, DamageKind::Fire);
    let text = POutcome::Played { label: "杀".into() }.text("杀");
    assert!(text.contains("使用【杀】"), "{text}");
}

// ---------------------------------------------------------------------------
// 普通锦囊
// ---------------------------------------------------------------------------

#[test]
fn wuzhong_draws_two() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "无中生有");
    let before = g.hand(0).len();
    play_simple(&mut g, 0, &card, vec![]);
    assert_eq!(g.hand(0).len(), before + 1, "用掉 1 张 + 摸 2 张");
    assert_eq!(g.zone_of(&card), Some(&Zone::Discard));
}

#[test]
fn wuzhong_negated_by_wuxie() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "无中生有");
    give(&mut g, 1, "无懈可击");
    let before = g.hand(0).len();
    play(&mut g, 0, &card, vec![], &mut |g| {
        assert_eq!(pend(g).actor(), 1);
        // 无懈浮层依赖 `pending.data.root/layers`（缺失会显示「被抵消的效果：（空）、0 层」）。
        assert!(
            pend(g).data()["root"]
                .as_str()
                .is_some_and(|r| r.contains("无中生有")),
            "浮层要显示被无懈的牌：{:?}",
            pend(g).data()["root"]
        );
        assert_eq!(pend(g).data()["layers"].as_u64(), Some(0));
        let w = opt_with(g, "respond:");
        answer(g, 1, &w);
    });
    assert_eq!(g.hand(0).len(), before - 1, "被无懈抵消则不摸牌");
}

#[test]
fn wuxie_chain_even_layers_restores_effect() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "无中生有");
    give(&mut g, 1, "无懈可击");
    give(&mut g, 2, "无懈可击");
    let before = g.hand(0).len();
    play(&mut g, 0, &card, vec![], &mut |g| {
        let w = opt_with(g, "respond:");
        answer(g, 1, &w);
        assert_eq!(pend(g).actor(), 2, "反无懈由 p2 给出");
        let w = opt_with(g, "respond:");
        answer(g, 2, &w);
    });
    assert_eq!(g.hand(0).len(), before + 1, "两层无懈 → 锦囊照常生效（摸 2 用 1）");
    assert!(
        g.log.iter().any(|e| e.text.contains("照常生效")),
        "文案必须是「照常生效」"
    );
}

#[test]
fn guohe_discards_chosen_card() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "过河拆桥");
    give(&mut g, 1, "桃");
    let before = g.hand(1).len();
    play(&mut g, 0, &card, vec![1], &mut |g| {
        assert_eq!(pend(g).actor(), 0, "由使用者选牌");
        answer(g, 0, "pick:hand");
    });
    assert_eq!(g.hand(1).len(), before - 1);
    assert_eq!(g.zone_of(&card), Some(&Zone::Discard));
}

#[test]
fn guohe_can_pick_equipment() {
    let mut g = new_game(3);
    let equip_card = equip(&mut g, 1, "八卦阵");
    let card = give(&mut g, 0, "过河拆桥");
    play(&mut g, 0, &card, vec![1], &mut |g| {
        answer(g, 0, &format!("pick:{equip_card}"));
    });
    assert_eq!(g.zone_of(&equip_card), Some(&Zone::Discard));
    assert_eq!(g.armor(1), ArmorKind::None);
}

#[test]
fn shunshou_gains_card_from_target() {
    let mut g = new_game(3);
    give(&mut g, 1, "桃");
    let card = give(&mut g, 0, "顺手牵羊");
    let before = g.hand(0).len();
    play(&mut g, 0, &card, vec![1], &mut |g| {
        answer(g, 0, "pick:hand");
    });
    assert_eq!(g.hand(0).len(), before, "用掉 1 张 + 获得 1 张");
    assert_eq!(g.hand(1).len(), 0);
}

#[test]
fn shunshou_requires_distance_one() {
    let mut g = new_game(4);
    let card = give(&mut g, 0, "顺手牵羊");
    give(&mut g, 2, "桃");
    let err = g.resolve_use(0, &card, vec![2]).unwrap_err();
    assert!(err.contains("距离"), "{err}");
}

#[test]
fn shunshou_rejects_target_without_cards() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "顺手牵羊");
    let err = g.resolve_use(0, &card, vec![1]).unwrap_err();
    assert!(err.contains("没有可操作"), "{err}");
}

#[test]
fn duel_hits_target_without_sha() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "决斗");
    let hp = g.seats[1].hp;
    play_simple(&mut g, 0, &card, vec![1]);
    assert_eq!(g.seats[1].hp, hp - 1);
}

#[test]
fn duel_alternates_until_someone_fails() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "决斗");
    give(&mut g, 1, "杀");
    give(&mut g, 0, "杀");
    let hp1 = g.seats[1].hp;
    play(&mut g, 0, &card, vec![1], &mut |g| {
        assert_eq!(pend(g).actor(), 1, "先由目标出杀");
        let sha = opt_with(g, "respond:");
        answer(g, 1, &sha);
        assert_eq!(pend(g).actor(), 0, "再由来源出杀");
        let sha = opt_with(g, "respond:");
        answer(g, 0, &sha);
        // p1 已无杀 → 直接结算。
        assert!(g.pending.is_none());
    });
    assert_eq!(g.seats[1].hp, hp1 - 1, "后手没杀 → 目标受伤");
}

#[test]
fn duel_source_without_sha_takes_damage() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "决斗");
    give(&mut g, 1, "杀");
    let hp0 = g.seats[0].hp;
    play(&mut g, 0, &card, vec![1], &mut |g| {
        let sha = opt_with(g, "respond:");
        answer(g, 1, &sha);
        assert!(g.pending.is_none(), "来源无杀 → 直接结算");
    });
    assert_eq!(g.seats[0].hp, hp0 - 1, "来源没杀 → 来源受伤");
}

#[test]
fn duel_target_fails_after_responding_once() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "决斗");
    give(&mut g, 1, "杀");
    give(&mut g, 0, "杀");
    let hp1 = g.seats[1].hp;
    play(&mut g, 0, &card, vec![1], &mut |g| {
        let sha = opt_with(g, "respond:");
        answer(g, 1, &sha);
        let sha = opt_with(g, "respond:");
        answer(g, 0, &sha);
        // p1 已无杀 → 直接结算，不再询问。
        assert!(g.pending.is_none());
    });
    assert_eq!(g.seats[1].hp, hp1 - 1, "后手没杀 → 目标受伤");
}

#[test]
fn nanman_hits_all_without_sha() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "南蛮入侵");
    let (h1, h2) = (g.seats[1].hp, g.seats[2].hp);
    play_simple(&mut g, 0, &card, vec![]);
    assert_eq!((g.seats[1].hp, g.seats[2].hp), (h1 - 1, h2 - 1));
}

#[test]
fn nanman_respects_sha_response_per_target() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "南蛮入侵");
    give(&mut g, 1, "杀");
    let (h1, h2) = (g.seats[1].hp, g.seats[2].hp);
    play(&mut g, 0, &card, vec![], &mut |g| {
        assert_eq!(pend(g).actor(), 1);
        let sha = opt_with(g, "respond:");
        answer(g, 1, &sha);
    });
    assert_eq!(g.seats[1].hp, h1, "出杀抵消");
    assert_eq!(g.seats[2].hp, h2 - 1);
}

#[test]
fn wanjian_asks_shan_per_target() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "万箭齐发");
    give(&mut g, 1, "闪");
    let (h1, h2) = (g.seats[1].hp, g.seats[2].hp);
    play(&mut g, 0, &card, vec![], &mut |g| {
        assert_eq!(pend(g).actor(), 1);
        let shan = opt_with(g, "respond:");
        answer(g, 1, &shan);
    });
    assert_eq!(g.seats[1].hp, h1, "闪抵消");
    assert_eq!(g.seats[2].hp, h2 - 1);
}

#[test]
fn taoyuan_heals_every_wounded_character() {
    let mut g = new_game(3);
    g.seats[0].hp = 2;
    g.seats[1].hp = 3;
    let card = give(&mut g, 0, "桃园结义");
    play_simple(&mut g, 0, &card, vec![]);
    assert_eq!((g.seats[0].hp, g.seats[1].hp, g.seats[2].hp), (3, 4, 4));
}

#[test]
fn wugu_reveals_and_each_picks_one() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "五谷丰登");
    let before: Vec<usize> = (0..3).map(|i| g.hand(i).len()).collect();
    play(&mut g, 0, &card, vec![], &mut |g| {
        for _ in 0..3 {
            let pick = opt_with(g, "pick:");
            let actor = pend(g).actor();
            answer(g, actor, &pick);
        }
    });
    // p0 用掉五谷（-1）又获得 1 张 → 净 0；p1/p2 各 +1。
    assert_eq!(g.hand(0).len(), before[0], "p0 净 0");
    assert_eq!(g.hand(1).len(), before[1] + 1, "p1 获得 1 张");
    assert_eq!(g.hand(2).len(), before[2] + 1, "p2 获得 1 张");
    assert_eq!(g.zone_of(&card), Some(&Zone::Discard));
}

#[test]
fn huogong_discards_same_suit_to_damage() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "火攻");
    let shown = give_suit(&mut g, 1, "桃", Suit::Heart);
    give_suit(&mut g, 0, "桃", Suit::Heart);
    let hp = g.seats[1].hp;
    play(&mut g, 0, &card, vec![1], &mut |g| {
        assert_eq!(pend(g).actor(), 1, "目标先展示手牌");
        answer(g, 1, &format!("reveal:{shown}"));
        assert_eq!(pend(g).actor(), 0, "再由使用者决定弃同花色");
        let d = opt_with(g, "discard:");
        answer(g, 0, &d);
    });
    assert_eq!(g.seats[1].hp, hp - 1, "火焰伤害 1");
    assert!(g.hand(1).contains(&shown), "展示不改变归属");
    assert!(!g.is_revealed(&shown), "展示结束");
}

#[test]
fn huogong_without_matching_suit_does_nothing() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "火攻");
    let shown = give_suit(&mut g, 1, "桃", Suit::Heart);
    give_suit(&mut g, 0, "杀", Suit::Spade);
    let hp = g.seats[1].hp;
    play(&mut g, 0, &card, vec![1], &mut |g| {
        answer(g, 1, &format!("reveal:{shown}"));
        assert!(g.pending.is_none(), "无同花色手牌 → 不询问");
    });
    assert_eq!(g.seats[1].hp, hp);
}

/// 回归（2026-11-11 真机）：火攻的「展示手牌」是强制步骤，目标不能取消整张火攻。
#[test]
fn huogong_reveal_is_mandatory() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "火攻");
    let shown = give_suit(&mut g, 1, "桃", Suit::Heart);
    give_suit(&mut g, 0, "桃", Suit::Heart);
    let hp = g.seats[1].hp;
    play(&mut g, 0, &card, vec![1], &mut |g| {
        assert_eq!(pend(g).actor(), 1);
        assert!(pend(g).escape_hidden(), "展示不是可放弃步骤（出口只能是占位）");
        assert_eq!(pend(g).data()["target"].as_str(), Some("p1"), "浮层显示目标");
        assert_eq!(pend(g).data()["stage"].as_str(), Some("reveal"));
        // 提交出口 = 自动展示第一张手牌（不是「火攻无效」）。
        answer(g, 1, "pass");
        assert_eq!(pend(g).actor(), 0, "展示后进入使用者弃同花色步骤");
        let d = opt_with(g, "discard:");
        answer(g, 0, &d);
    });
    assert_eq!(g.seats[1].hp, hp - 1, "火攻照常造成 1 点火焰伤害");
    assert!(g.hand(1).contains(&shown));
}

/// 回归（2026-11-11 真机）：五谷的待办必须带上 `pool`（视图 `special_zones.wugu` 读 pending.data），
/// 否则端侧五谷特殊区没有牌可点 → 对局卡住。
#[test]
fn harvest_pending_carries_pool_for_view() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "五谷丰登");
    g.resolve_use(0, &card, vec![]).expect("使用五谷");
    let pool: Vec<String> = pend(&g).data()["pool"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    assert_eq!(pool.len(), 3, "三名角色 → 三张展示牌进入 pending.data.pool");
    assert_eq!(pend(&g).data()["cursor"].as_u64(), Some(0));
    assert_eq!(pend(&g).data()["pickers"].as_array().map(|a| a.len()), Some(3));
    // 端侧特殊区投影：`special_zones.wugu.cards` 必须非空（否则五谷界面无按钮、对局卡住）。
    let view = sanguosha2::view::player_view(&g, 0);
    let wugu_cards = view["special_zones"]["wugu"]["cards"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(wugu_cards.len(), 3, "五谷特殊区应展示 3 张候选：{view}");
    assert_eq!(
        view["special_zones"]["wugu"]["current"].as_str().is_some(),
        true,
        "五谷特殊区应标出当前选牌者"
    );
    // 旧存档兜底：`pending.data` 缺失（旧实现写入）时，仍要能从选项反推展示牌。
    let mut legacy = serde_json::to_value(g.pending.as_ref().unwrap()).unwrap();
    legacy["data"] = serde_json::Value::Null;
    g.pending = Some(serde_json::from_value(legacy).unwrap());
    let legacy_view = sanguosha2::view::player_view(&g, 0);
    assert_eq!(
        legacy_view["special_zones"]["wugu"]["cards"]
            .as_array()
            .map(|a| a.len()),
        Some(3),
        "pending.data 缺失时也不能让五谷特殊区空掉"
    );
}

/// 回归（2026-11-11 真机）：同一张【杀】对不同目标的合法动作，标签必须能区分目标
/// （否则端侧「选择目标 / 用法」浮层里全是「使用【杀】」）。
#[test]
fn sha_action_labels_include_targets() {
    let mut g = new_game(3);
    g.seats[1].hp = 3;
    give(&mut g, 0, "杀");
    let acts = sanguosha2::kernel::card_play::legal_uses(&g, 0);
    let sha: Vec<&str> = acts
        .iter()
        .filter(|(id, _)| id.starts_with("use:"))
        .map(|(_, label)| label.as_str())
        .collect();
    assert!(!sha.is_empty(), "应能使用【杀】：{acts:?}");
    assert!(
        sha.iter().all(|l| l.contains("→")),
        "【杀】的选项必须带目标名：{sha:?}"
    );
    assert_eq!(
        sha.iter().filter(|l| l.contains("p1")).count(),
        1,
        "目标 p1 的选项唯一：{sha:?}"
    );
    assert_eq!(
        sha.iter().filter(|l| l.contains("p2")).count(),
        1,
        "目标 p2 的选项唯一：{sha:?}"
    );
}

/// 回归（2026-11-11 同类排查）：五谷取牌是**强制**的（每名目标角色依次获得一张），
/// 提交出口 = 自动取第一张（不是「放弃不取」）。
#[test]
fn harvest_pick_is_mandatory() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "五谷丰登");
    g.resolve_use(0, &card, vec![]).expect("使用五谷");
    let mut guard = 0;
    while pend(&g).kind() != "harvest" && guard < 8 {
        let actor = pend(&g).actor();
        answer(&mut g, actor, "pass");
        guard += 1;
    }
    assert_eq!(pend(&g).kind(), "harvest");
    let picker = pend(&g).actor();
    assert!(pend(&g).escape_hidden(), "五谷取牌不可拒绝（出口只是占位）");
    let before = g.hand(picker).len();
    answer(&mut g, picker, "pass"); // 占位提交 = 取第一张
    assert_eq!(g.hand(picker).len(), before + 1, "占位提交应真的取到一张牌");
}

/// 回归（2026-11-11 同类排查）：拆/顺的取牌浮层依赖 `pending.data`（目标/牌名），
/// 且选牌是**强制**步骤（出口只能是占位）。
#[test]
fn dismantle_pending_carries_data_and_is_mandatory() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "过河拆桥");
    let weapon = equip(&mut g, 1, "青龙偃月刀");
    play(&mut g, 0, &card, vec![1], &mut |g| {
        assert_eq!(pend(g).kind(), "card_pick");
        assert!(pend(g).escape_hidden(), "拆/顺选牌不可取消（出口只是占位）");
        assert_eq!(pend(g).data()["target"].as_str(), Some("p1"), "浮层要显示目标");
        assert_eq!(pend(g).data()["card"].as_str(), Some("过河拆桥"));
        // 提交出口 = 自动选第一张（= 武器），不会「取消整张牌」。
        answer(g, 0, "pass");
    });
    assert_eq!(
        g.zone_of(&weapon),
        Some(&Zone::Discard),
        "占位提交应真的弃掉一张牌（不是让整张拆无效）"
    );
}

/// 回归（2026-11-11 同类排查）：**同一张牌对不同目标的选项标签必须可区分**。
///
/// 端侧（Web「选择目标 / 用法」浮层、QQ 清单）直接渲染 `description`；
/// 若两个选项文案相同，玩家无法判断会打谁（真机：一屏「使用【杀】」）。
#[test]
fn legal_use_labels_are_unique_and_name_targets() {
    let mut g = new_game(4);
    g.seats[0].hp = 3; // 让【桃】可选
    for seat in 1..4 {
        give(&mut g, seat, "闪");
    }
    for name in [
        "杀", "桃", "决斗", "过河拆桥", "顺手牵羊", "火攻", "借刀杀人", "五谷丰登", "铁索连环",
        "无中生有", "万箭齐发", "桃园结义",
    ] {
        give(&mut g, 0, name);
    }
    equip(&mut g, 0, "青龙偃月刀");
    let acts = sanguosha2::kernel::card_play::legal_uses(&g, 0);
    let mut seen: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for (id, label) in &acts {
        if let Some(prev) = seen.insert(label.as_str(), id.as_str()) {
            panic!(
                "选项文案重复，端侧无法区分：
  {label}
   ← {prev}
   ← {id}"
            );
        }
    }
    // 同一张牌产生多个选项时（= 有多种目标组合），每个标签都必须带目标名。
    let mut by_card: std::collections::HashMap<String, Vec<&str>> = std::collections::HashMap::new();
    for (id, label) in &acts {
        let base = id.split(':').take(2).collect::<Vec<_>>().join(":");
        by_card.entry(base).or_default().push(label.as_str());
    }
    for (base, labels) in by_card {
        if labels.len() > 1 {
            let missing = labels.iter().filter(|l| !l.contains("→")).count();
            assert_eq!(
                missing, 0,
                "同一张牌有 {} 种用法，但有 {missing} 个选项没写目标名：{base} → {labels:?}",
                labels.len()
            );
        }
    }
}

#[test]
fn jiedao_target_uses_sha_on_second_target() {
    let mut g = new_game(3);
    equip(&mut g, 1, "青龙偃月刀");
    give(&mut g, 1, "杀");
    let card = give(&mut g, 0, "借刀杀人");
    let hp = g.seats[2].hp;
    play(&mut g, 0, &card, vec![1, 2], &mut |g| {
        assert_eq!(pend(g).actor(), 1);
        let sha = opt_with(g, "respond:");
        answer(g, 1, &sha);
    });
    assert_eq!(g.seats[2].hp, hp - 1, "A 对 B 使用杀");
    assert_eq!(g.zone_of(&card), Some(&Zone::Discard));
}

#[test]
fn jiedao_refusal_gives_weapon_to_user() {
    let mut g = new_game(3);
    let weapon = equip(&mut g, 1, "青龙偃月刀");
    let card = give(&mut g, 0, "借刀杀人");
    play(&mut g, 0, &card, vec![1, 2], &mut |g| {
        answer(g, 1, "pass");
    });
    assert!(g.hand(0).contains(&weapon), "拒绝出杀 → 交出武器");
    assert_eq!(g.equip_in(1, sanguosha2::kernel::Slot::Weapon), None);
}

#[test]
fn jiedao_requires_weapon_on_first_target() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "借刀杀人");
    let err = g.resolve_use(0, &card, vec![1, 2]).unwrap_err();
    assert!(err.contains("武器"), "{err}");
}

#[test]
fn tiesuo_toggles_chain_state() {
    let mut g = new_game(3);
    g.seats[1].chained = true;
    let card = give(&mut g, 0, "铁索连环");
    play_simple(&mut g, 0, &card, vec![0, 1]);
    assert!(g.seats[0].chained, "未横置 → 横置");
    assert!(!g.seats[1].chained, "已横置 → 重置");
}

#[test]
fn tiesuo_recast_discards_and_draws_without_counting_use() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "铁索连环");
    let before = g.hand(0).len();
    let used = g.counter("turn.cards_used");
    sanguosha2::kernel::card_play::resolve_recast(&mut g, 0, &card).unwrap();
    assert_eq!(g.hand(0).len(), before, "弃 1 摸 1");
    assert_eq!(g.counter("turn.cards_used"), used, "重铸不计入使用次数");
    assert_eq!(g.zone_of(&card), Some(&Zone::Discard));
}

#[test]
fn non_lianhuan_cannot_be_recast() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "杀");
    assert!(sanguosha2::kernel::card_play::resolve_recast(&mut g, 0, &card).is_err());
}

// ---------------------------------------------------------------------------
// 延时锦囊 + 判定阶段（LIFO）
// ---------------------------------------------------------------------------

#[test]
fn lebu_enters_judge_zone_and_does_not_resolve_on_use() {
    let mut g = new_game(3);
    let le = give(&mut g, 0, "乐不思蜀");
    g.resolve_use(0, &le, vec![1]).unwrap();
    assert_eq!(g.judge_zone(1), vec![le.clone()]);
    assert_eq!(g.seats[1].hp, 4, "延时锦囊使用时不结算");
}

#[test]
fn lebu_skips_play_phase_when_judge_not_heart() {
    let mut g = new_game(3);
    let le = give(&mut g, 0, "乐不思蜀");
    g.resolve_use(0, &le, vec![1]).unwrap();
    put_top_where(&mut g, |c| c.suit != Suit::Heart);
    begin_phase(&mut g, 1, Phase::Judge).unwrap();
    invariant(&g);
    assert_eq!(g.judge_zone(1), Vec::<CardId>::new(), "判定后离开判定区");
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    assert_eq!(g.phase, Phase::Discard, "跳过出牌阶段");
}

#[test]
fn lebu_does_not_skip_when_judge_is_heart() {
    let mut g = new_game(3);
    let le = give(&mut g, 0, "乐不思蜀");
    g.resolve_use(0, &le, vec![1]).unwrap();
    put_top_where(&mut g, |c| c.suit == Suit::Heart);
    begin_phase(&mut g, 1, Phase::Judge).unwrap();
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    assert_eq!(g.phase, Phase::Play, "红桃判定不跳过");
}

#[test]
fn bingliang_skips_draw_phase_when_not_club() {
    let mut g = new_game(3);
    let bing = give(&mut g, 0, "兵粮寸断");
    g.resolve_use(0, &bing, vec![1]).unwrap();
    put_top_where(&mut g, |c| c.suit != Suit::Club);
    begin_phase(&mut g, 1, Phase::Judge).unwrap();
    assert!(g.draw_phase(1).is_empty(), "跳过摸牌阶段");
}

#[test]
fn bingliang_does_not_skip_on_club() {
    let mut g = new_game(3);
    let bing = give(&mut g, 0, "兵粮寸断");
    g.resolve_use(0, &bing, vec![1]).unwrap();
    put_top_where(&mut g, |c| c.suit == Suit::Club);
    begin_phase(&mut g, 1, Phase::Judge).unwrap();
    assert_eq!(g.draw_phase(1).len(), 2, "梅花判定不跳过摸牌");
}

#[test]
fn delayed_trick_duplicate_on_same_target_rejected() {
    let mut g = new_game(3);
    let a = give(&mut g, 0, "乐不思蜀");
    g.resolve_use(0, &a, vec![1]).unwrap();
    let b = give(&mut g, 0, "乐不思蜀");
    let err = g.resolve_use(0, &b, vec![1]).unwrap_err();
    assert!(err.contains("已有同名"), "{err}");
}

#[test]
fn shandian_deals_three_thunder_damage() {
    let mut g = new_game(3);
    let ld = give(&mut g, 0, "闪电");
    g.resolve_use(0, &ld, vec![]).unwrap();
    assert_eq!(g.judge_zone(0), vec![ld.clone()]);
    put_top_where(&mut g, |c| c.suit == Suit::Spade && (2..=9).contains(&c.rank));
    let hp = g.seats[0].hp;
    begin_phase(&mut g, 0, Phase::Judge).unwrap();
    invariant(&g);
    assert_eq!(g.seats[0].hp, hp - 3, "闪电 3 点雷电伤害");
    assert_eq!(g.zone_of(&ld), Some(&Zone::Discard));
}

#[test]
fn shandian_moves_to_next_when_judge_misses() {
    let mut g = new_game(3);
    let ld = give(&mut g, 0, "闪电");
    g.resolve_use(0, &ld, vec![]).unwrap();
    put_top_where(&mut g, |c| c.suit == Suit::Heart);
    begin_phase(&mut g, 0, Phase::Judge).unwrap();
    assert_eq!(g.judge_zone(0), Vec::<CardId>::new());
    assert_eq!(g.judge_zone(1), vec![ld.clone()], "移交给下家");
    assert_eq!(g.seats[0].hp, 4, "未命中不受伤");
}

#[test]
fn judge_zone_is_lifo() {
    let mut g = new_game(3);
    let le = give(&mut g, 0, "乐不思蜀");
    let bing = give(&mut g, 0, "兵粮寸断");
    g.to_judge(&le, 1).unwrap();
    g.to_judge(&bing, 1).unwrap();
    assert_eq!(g.judge_zone_lifo(1), vec![bing, le], "后置先判");
}

#[test]
fn judge_lifo_resolves_later_placed_first() {
    let mut g = new_game(3);
    let le = give(&mut g, 0, "乐不思蜀");
    let bing = give(&mut g, 0, "兵粮寸断");
    g.to_judge(&le, 1).unwrap();
    g.to_judge(&bing, 1).unwrap();
    // 两张判定牌都非梅花/非红桃 → 依次触发「跳过摸牌」再「跳过出牌」。
    // 注意用不同花色，避免两次 `put_top_where` 命中同一张牌。
    put_top_where(&mut g, |c| c.suit == Suit::Diamond);
    put_top_where(&mut g, |c| c.suit == Suit::Spade);
    begin_phase(&mut g, 1, Phase::Judge).unwrap();
    let draw_pos = g
        .log
        .iter()
        .position(|e| e.text.contains("跳过摸牌阶段"))
        .expect("应有兵粮判定");
    let play_pos = g
        .log
        .iter()
        .position(|e| e.text.contains("跳过出牌阶段"))
        .expect("应有乐不思蜀判定");
    assert!(draw_pos < play_pos, "LIFO：后置的兵粮先结算");
}

#[test]
fn lebu_can_be_negated_by_wuxie_in_judge_phase() {
    let mut g = new_game(3);
    let le = give(&mut g, 0, "乐不思蜀");
    g.resolve_use(0, &le, vec![1]).unwrap();
    give(&mut g, 2, "无懈可击");
    put_top_where(&mut g, |c| c.suit != Suit::Heart);
    begin_phase(&mut g, 1, Phase::Judge).unwrap();
    assert_eq!(pend(&g).actor(), 2, "无懈窗口由 p2 决定");
    let w = opt_with(&g, "respond:");
    answer(&mut g, 2, &w);
    invariant(&g);
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    assert_eq!(g.phase, Phase::Play, "被无懈抵消 → 不跳过出牌阶段");
    assert_eq!(g.zone_of(&le), Some(&Zone::Discard));
}

// ---------------------------------------------------------------------------
// 牌区不变量 / 门禁
// ---------------------------------------------------------------------------

#[test]
fn every_resolution_keeps_conservation_and_no_orphan_frames() {
    let mut g = new_game(4);
    let cards_to_play = [
        "无中生有",
        "过河拆桥",
        "决斗",
        "南蛮入侵",
        "万箭齐发",
        "桃园结义",
        "五谷丰登",
    ];
    // 先把所有牌拿到手里：避免「无中生有/五谷」把后面要用的牌摸走。
    let hand: Vec<(CardId, bool)> = cards_to_play
        .iter()
        .map(|name| {
            let id = give(&mut g, 0, name);
            let needs_target =
                g.def_of(&id).tags.target_mode == Some(cards::TargetMode::Chosen);
            (id, needs_target)
        })
        .collect();
    for (id, needs_target) in hand {
        give(&mut g, 1, "桃"); // 保证 过河拆桥/顺手 有合法目标
        let target = if needs_target { vec![1] } else { vec![] };
        g.resolve_use(0, &id, target)
            .unwrap_or_else(|e| panic!("使用牌 {id} 失败：{e}"));
        invariant(&g);
        let mut guard = 0;
        while let Some(p) = g.pending.clone() {
            guard += 1;
            assert!(guard < 50, "pending 未收敛");
            let escape = p.escape_id().to_string();
            answer(&mut g, p.actor(), &escape);
        }
        invariant(&g);
        // 清掉可能阻塞下张牌的 pending（如五谷放弃、决斗放弃）。
        g.pending = None;
    }
}

#[test]
fn gate_card_play_never_touches_hp_directly() {
    let src = include_str!("../src/kernel/card_play.rs");
    for pat in [".hp +=", ".hp -=", ".hp ="] {
        assert!(!src.contains(pat), "牌结算不得直接改体力（命中 {pat}）：请走 flow::*");
    }
}

#[test]
fn gate_kernel_has_no_card_name_branch() {
    for (name, src) in [
        ("kernel/mod.rs", include_str!("../src/kernel/mod.rs")),
        ("kernel/resolve.rs", include_str!("../src/kernel/resolve.rs")),
        ("kernel/flow.rs", include_str!("../src/kernel/flow.rs")),
        ("kernel/deferred.rs", include_str!("../src/kernel/deferred.rs")),
        ("kernel/card_play.rs", include_str!("../src/kernel/card_play.rs")),
    ] {
        // 只禁「判定」用法：牌名比较/构造。展示文案里的牌名（日志/提示）不算。
        for card_name in [
            "== \"杀\"",
            "== \"闪\"",
            "== \"桃\"",
            "== \"酒\"",
            "== \"决斗\"",
            "== \"无懈可击\"",
            "== \"南蛮入侵\"",
            "== \"万箭齐发\"",
            "== \"乐不思蜀\"",
            "== \"兵粮寸断\"",
            "== \"闪电\"",
            "== \"铁索连环\"",
            "cards::def(\"",
        ] {
            assert!(
                !src.contains(card_name),
                "{name} 出现按牌名判定的写法（{card_name}）：请读 cards::def() 属性"
            );
        }
    }
}

#[test]
fn gate_kernel_routes_damage_through_flow() {
    let src = include_str!("../src/kernel/card_play.rs");
    assert!(
        src.contains("flow::damage_with") || src.contains("flow::damage_typed"),
        "牌结算必须走 flow 伤害出口"
    );
    assert!(!src.contains("fn damage("), "牌结算不得自建伤害入口");
}

// ---------------------------------------------------------------------------
// 牌型覆盖门禁（每牌型 ≥1 正例）
// ---------------------------------------------------------------------------

#[test]
fn gate_every_effect_has_a_dispatch_branch() {
    let play = include_str!("../src/kernel/card_play.rs");
    for def in sanguosha2::cards_generated::DEFS {
        if def.effect == Effect::None {
            assert_eq!(
                def.kind,
                cards::CardKind::Equip,
                "牌 {} 缺少效果却不是装备",
                def.name
            );
            continue;
        }
        // 闪 / 无懈可击只能响应：由 `response_options` 与 `flow::step_wuxie` 处理，不在主动结算分支里。
        if matches!(def.effect, Effect::Shan | Effect::Wuxie) {
            continue;
        }
        let pat = format!("Effect::{:?}", def.effect);
        assert!(
            play.contains(&pat),
            "牌型 {pat}（{}）没有结算分支",
            def.name
        );
    }
}

#[test]
fn gate_all_deck_defs_are_covered_by_effects_or_equip() {
    for def in sanguosha2::cards_generated::DEFS {
        if def.kind == cards::CardKind::Equip {
            assert!(def.slot.is_some(), "装备 {} 缺少槽位", def.name);
        } else {
            assert_ne!(def.effect, Effect::None, "牌 {} 缺少效果", def.name);
        }
    }
}
