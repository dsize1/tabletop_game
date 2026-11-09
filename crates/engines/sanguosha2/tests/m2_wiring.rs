//! M2 接线复查用例：B0–B7 里原本「`use_card`/`use_virtual` 打桩」的技能，改走真实结算后
//! 仍保持原语义，并且**不会**因为真实结算产生新的误触发。
//!
//! 覆盖（`memory_palace/plan/next-engine-replacement.md` §M2）：
//! B3 奇策；B4 慎断；B5 活墨；B6 战绝/勤王；B7 滔乱/止戈。
//! 另含门禁：`src/skills/**` 不得再调用事件模拟入口 `use_card`。

use std::path::Path;

use sanguosha2::cards;
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{CardId, Game, Phase, Seat, Zone};
use sanguosha2::skills;

// ---------------------------------------------------------------------------
// 夹具
// ---------------------------------------------------------------------------

fn game(specs: &[(&str, &[&str], i32)]) -> Game {
    let seats: Vec<Seat> = specs
        .iter()
        .map(|(n, s, hp)| Seat::new(n, "gen", s, *hp))
        .collect();
    Game::new(seats, cards::deck())
}

fn give(g: &mut Game, seat: usize, name: &str) -> CardId {
    let id = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| g.name_of(id) == name)
        .unwrap_or_else(|| panic!("牌堆里没有【{name}】"));
    g.to_hand(&id, seat).expect("给牌");
    id
}

fn pend(g: &Game) -> &sanguosha2::kernel::pending::Pending {
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
    g.check_conservation().expect("守恒");
    g.check_no_orphan_frames().expect("无帧遗留");
}

fn play(g: &mut Game, seat: usize) {
    begin_phase(g, seat, Phase::Play).unwrap();
}

fn log_text(g: &Game) -> String {
    g.log
        .iter()
        .map(|l| l.text.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// B3 奇策：把全部手牌当普通锦囊 → 真实结算（摸牌/伤害/无懈）
// ---------------------------------------------------------------------------

#[test]
fn qice_as_exnihilo_really_draws_two() {
    let mut g = game(&[("xun", &["qice"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, "杀");
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:qice").unwrap();
    answer(&mut g, 0, "choose:无中生有");
    // 真实结算：origin（承载牌会先被移出，结算后整组进弃牌堆）→ 摸 2。
    assert_eq!(g.hand(0).len(), 2, "无中生有真实摸 2");
    assert!(g.pending.is_none());
}

#[test]
fn qice_as_duel_really_damages_target() {
    let mut g = game(&[("xun", &["qice"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, "杀");
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:qice").unwrap();
    answer(&mut g, 0, "choose:决斗");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 3, "决斗真实造成 1 点伤害");
}

#[test]
fn qice_can_be_negated_by_wuxie() {
    let mut g = game(&[("xun", &["qice"], 4), ("p1", &[], 4), ("p2", &[], 4)]);
    give(&mut g, 0, "杀");
    give(&mut g, 2, "无懈可击");
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:qice").unwrap();
    answer(&mut g, 0, "choose:无中生有");
    assert_eq!(pend(&g).actor(), 2, "无懈窗口");
    let w = opt_with(&g, "respond:");
    answer(&mut g, 2, &w);
    assert_eq!(g.hand(0).len(), 0, "被无懈抵消 → 不摸牌");
    assert!(log_text(&g).contains("被【无懈可击】抵消"));
}

// ---------------------------------------------------------------------------
// B4 慎断：弃牌堆里的黑色基本牌当无距离【兵粮寸断】→ 真实进判定区
// ---------------------------------------------------------------------------

#[test]
fn shenduan_bingliang_ignores_distance_and_enters_judge_zone() {
    let mut g = game(&[
        ("hanhao", &["shenduan"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
        ("p3", &[], 4),
    ]);
    play(&mut g, 0);
    // 慎断只接「黑色基本牌」：从牌堆取一张黑色【杀】。
    let card = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| g.name_of(id) == "杀" && !g.is_red(id))
        .expect("黑色杀");
    g.to_hand(&card, 0).unwrap();
    g.to_discard(&card, "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("shenduan"));
    answer(&mut g, 0, "choose:shenduan");
    answer(&mut g, 0, "target:2"); // 距离 2，无距离限制
    assert_eq!(g.judge_zone(2), vec![card.clone()], "真实进入判定区");

    // 转化延时锦囊在判定阶段仍按【兵粮寸断】生效（虚拟定义保留到判定结束）。
    g.put_on_top(
        &g.ids_in(|z| *z == Zone::Draw)
            .into_iter()
            .find(|id| g.cards.get(id).map(|c| c.suit) != Some(sanguosha2::kernel::Suit::Club))
            .expect("非梅花判定牌"),
    )
    .unwrap();
    begin_phase(&mut g, 2, Phase::Judge).unwrap();
    assert!(g.judge_zone(2).is_empty(), "判定后离开判定区");
    assert!(g.draw_phase(2).is_empty(), "非梅花 → 跳过摸牌阶段");
}

// ---------------------------------------------------------------------------
// B5 活墨：视为使用基本牌 → 真实选目标 + 酒加成
// ---------------------------------------------------------------------------

#[test]
fn huomo_sha_selects_target_and_deals_damage() {
    let mut g = game(&[("zhongyao", &["huomo"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, "决斗"); // 黑色非基本牌
    play(&mut g, 0);
    let opts = {
        skills::execute_active(&mut g, 0, "skill:huomo").unwrap();
        options(&g)
    };
    assert!(opts.iter().any(|o| o.ends_with(":杀")), "{opts:?}");
    let choose = opts
        .iter()
        .find(|o| o.ends_with(":杀"))
        .cloned()
        .unwrap();
    answer(&mut g, 0, &choose);
    assert_eq!(pend(&g).actor(), 0, "活墨【杀】需要选目标");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 3, "视为使用【杀】真实造成伤害");
}

#[test]
fn huomo_jiu_applies_next_sha_bonus() {
    let mut g = game(&[("zhongyao", &["huomo"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, "决斗"); // 黑色非基本牌
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:huomo").unwrap();
    let opts = options(&g);
    let choice = opts
        .iter()
        .find(|o| o.ends_with(":酒"))
        .cloned()
        .unwrap_or_else(|| panic!("缺少酒选项：{opts:?}"));
    answer(&mut g, 0, &choice);
    assert_eq!(g.counter("turn.jiu_bonus"), 1, "酒加成已置");
    assert_eq!(g.counter("turn.jiu_used:0"), 1, "每回合一次的同一入口");
    // 下一张【杀】按酒加成结算（真实伤害 2）。
    let sha = give(&mut g, 0, "杀");
    let hp = g.seats[1].hp;
    g.resolve_use(0, &sha, vec![1]).unwrap();
    assert_eq!(g.seats[1].hp, hp - 2, "酒：下一张杀伤害 +1");
}

// ---------------------------------------------------------------------------
// B6 战绝 / 勤王：真实决斗 / 真实代出【杀】
// ---------------------------------------------------------------------------

#[test]
fn zhanjue_as_duel_can_be_negated_by_wuxie() {
    let mut g = game(&[("liuchen", &["zhanjue"], 4), ("p1", &[], 4), ("p2", &[], 4)]);
    give(&mut g, 0, "闪");
    give(&mut g, 2, "无懈可击");
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:zhanjue").unwrap();
    answer(&mut g, 0, "choose:zhanjue");
    answer(&mut g, 0, "target:1");
    assert_eq!(pend(&g).actor(), 2, "决斗的无懈窗口");
    let w = opt_with(&g, "respond:");
    answer(&mut g, 2, &w);
    assert_eq!(g.seats[1].hp, 4, "被无懈抵消 → 不造成伤害");
    assert!(g.hand(0).is_empty(), "origin 整组仍进弃牌堆");
}

#[test]
fn qinwang_ally_sha_damage_source_is_lord() {
    let mut g = game(&[("liuchen", &["qinwang"], 4), ("ally", &[], 4)]);
    g.seats[0].flags.insert("lord".to_string());
    g.seats[1].kingdom = g.seats[0].kingdom.clone();
    let weapon = give(&mut g, 0, "青龙偃月刀");
    g.to_equip(&weapon, 0, sanguosha2::kernel::Slot::Weapon)
        .unwrap();
    give(&mut g, 1, "杀");
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:qinwang").unwrap();
    answer(&mut g, 0, &format!("choose:{weapon}"));
    answer(&mut g, 0, "target:1");
    let pick = opt_with(&g, "choose:");
    answer(&mut g, 1, &pick);
    assert_eq!(g.seats[0].hp, 3, "代出的【杀】命中刘谌");
    assert!(
        log_text(&g).contains("liuchen 对 liuchen"),
        "伤害来源视为刘谌：{}",
        log_text(&g)
    );
}

// ---------------------------------------------------------------------------
// B7 滔乱 / 止戈：真实响应与真实【杀】
// ---------------------------------------------------------------------------

#[test]
fn taoluan_as_sha_asks_for_shan() {
    let mut g = game(&[("zhangrang", &["taoluan"], 4), ("p1", &[], 4), ("p2", &[], 4)]);
    give(&mut g, 0, "闪");
    give(&mut g, 1, "闪");
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:taoluan").unwrap();
    let sha_choice = options(&g)
        .into_iter()
        .find(|o| o.ends_with(":杀"))
        .expect("滔乱可声明【杀】");
    answer(&mut g, 0, &sha_choice);
    answer(&mut g, 0, "cardtarget:1");
    assert_eq!(pend(&g).actor(), 1, "滔乱【杀】要求目标响应");
    let shan = opt_with(&g, "respond:");
    let hp = g.seats[1].hp;
    answer(&mut g, 1, &shan);
    assert_eq!(g.seats[1].hp, hp, "闪抵消滔乱的【杀】");
}

#[test]
fn taoluan_used_name_is_not_offered_again_even_after_response() {
    let mut g = game(&[("zhangrang", &["taoluan"], 4), ("p1", &[], 4), ("p2", &[], 4)]);
    give(&mut g, 0, "闪");
    give(&mut g, 1, "闪");
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:taoluan").unwrap();
    let first = opt_with(&g, "choose:");
    let id = first
        .strip_prefix("choose:")
        .unwrap()
        .rsplit_once(':')
        .unwrap()
        .0
        .to_string();
    answer(&mut g, 0, &format!("choose:{id}:杀"));
    answer(&mut g, 0, "cardtarget:1");
    let shan = opt_with(&g, "respond:");
    answer(&mut g, 1, &shan);
    // 结算并完成滔乱的后续询问（opt1/opt2）。
    answer(&mut g, 0, "pass");
    give(&mut g, 0, "桃");
    skills::execute_active(&mut g, 0, "skill:taoluan").unwrap();
    assert!(
        !options(&g).iter().any(|o| o.ends_with(":杀")),
        "用过的牌名整局限一次：{:?}",
        options(&g)
    );
    answer(&mut g, 0, "pass");
}

// ---------------------------------------------------------------------------
// 门禁：技能不得再走事件模拟入口
// ---------------------------------------------------------------------------

#[test]
fn gate_skills_use_real_resolution_only() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/skills");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("读 skills 目录") {
        let path = entry.expect("目录项").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("读源文件");
        assert!(
            !src.contains("use_card("),
            "{} 仍调用事件模拟入口 `use_card`：请改用 resolve_use / use_virtual / use_as",
            path.display()
        );
        checked += 1;
    }
    assert!(checked > 10, "应扫描到全部技能文件");
}

#[test]
fn gate_virtual_use_counts_toward_sha_limit() {
    // 纯虚拟【杀】占用本阶段次数（sha_used 派生）。
    let mut g = game(&[("a", &[], 4), ("p1", &[], 4)]);
    play(&mut g, 0);
    g.use_virtual(0, "杀", vec![1]).unwrap();
    advance(&mut g).unwrap();
    assert_eq!(g.sha_used(0), 1, "虚拟杀计入次数");
}
