def patch(path, pairs):
    s = open(path, encoding='utf-8').read()
    for old, new in pairs:
        if old not in s:
            raise SystemExit("MISSING in %s:\n%s" % (path, old[:300]))
        s = s.replace(old, new, 1)
    open(path, 'w', encoding='utf-8').write(s)

# ---------------- b9：铁骑逐目标 ----------------
patch('src/kernel/skills/b9.rs', [
    ("""        "sha_targeted" => {
            match take_answer(g, "tieqi") {
                None => {
                    let mut b = PendingBuilder::new(actor, "choose").skill("tieqi");
                    b = b.option("use", "铁骑：令目标本回合非锁定技失效并判定");
                    Ok(Step::Need(
                        b.escape("不发动")
                            .data(json!({ "step": "sha_targeted", "card": card, "targets": targets }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "use" => {
                    for t in &targets {
                        g.bump(&format!("suppress.{t}.{}", g.turn));
                    }
                    let Some(t) = targets.first().copied() else {
                        return Ok(Step::Done);
                    };
                    let r = judge::judge(g, actor, "铁骑").map(|r| r.suit);
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "judged", "card": card, "target": t, "suit": format!("{:?}", r) }),
                    ))
                }
                Some(o) => Err(format!("铁骑：未知选项 {o}")),
            }
        }
        "judged" => {
            let t = common::seat_of(ask, "target").ok_or("铁骑：缺目标")?;
            let suit = ask.data.get("suit").and_then(|v| v.as_str()).unwrap_or("Spade").to_string();
            let want = match suit.as_str() {
                "Spade" => Suit::Spade,
                "Heart" => Suit::Heart,
                "Club" => Suit::Club,
                _ => Suit::Diamond,
            };
            let cands: Vec<CardId> = own_cards(g, t)
                .into_iter()
                .filter(|id| g.card_defs.get(id).map(|c| c.suit == want).unwrap_or(false))
                .collect();
            match take_answer(g, "tieqi") {
                None => {
                    let mut b = PendingBuilder::new(t, "pick_own").skill("tieqi");
                    for id in &cands {
                        b = b.option(format!("card:{id}"), format!("弃置【{}】抵消铁骑", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape("不弃置（不能使用【闪】）")
                            .data(json!({ "step": "judged", "card": card, "target": t, "suit": suit }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => {
                    super::super::card_play::declare_intercept(
                        g,
                        &card,
                        super::super::ShaIntercept::Unresponsive,
                    );
                    Ok(Step::Finish("铁骑：其不弃置同花色牌，不能使用【闪】".into()))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("铁骑：只能弃置同花色的手牌/装备".into());
                    }
                    flow::discard(g, t, &id, "铁骑");
                    Ok(Step::Finish("铁骑：其弃置同花色牌".into()))
                }
                Some(o) => Err(format!("铁骑：未知选项 {o}")),
            }
        }""",
     """        "sha_targeted" => {
            match take_answer(g, "tieqi") {
                None => {
                    let mut b = PendingBuilder::new(actor, "choose").skill("tieqi");
                    b = b.option("use", "铁骑：令目标本回合非锁定技失效并判定");
                    Ok(Step::Need(
                        b.escape("不发动")
                            .data(json!({ "step": "sha_targeted", "card": card, "targets": targets }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "use" => {
                    for t in &targets {
                        g.bump(&format!("suppress.{t}.{}", g.turn));
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "judge", "card": card, "targets": targets, "idx": 0 }),
                    ))
                }
                Some(o) => Err(format!("铁骑：未知选项 {o}")),
            }
        }
        "judge" => {
            let targets = common::seats_of(ask, "targets");
            let idx = ask.data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if idx >= targets.len() {
                return Ok(Step::Finish("铁骑：结算完成".into()));
            }
            let t = targets[idx];
            let r = judge::judge(g, actor, "铁骑").map(|r| r.suit);
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "judged", "card": card, "target": t, "idx": idx, "targets": targets, "suit": format!("{:?}", r) }),
            ))
        }
        "judged" => {
            let t = common::seat_of(ask, "target").ok_or("铁骑：缺目标")?;
            let idx = ask.data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let targets = common::seats_of(ask, "targets");
            let suit = ask.data.get("suit").and_then(|v| v.as_str()).unwrap_or("Spade").to_string();
            let want = match suit.as_str() {
                "Spade" => Suit::Spade,
                "Heart" => Suit::Heart,
                "Club" => Suit::Club,
                _ => Suit::Diamond,
            };
            let cands: Vec<CardId> = own_cards(g, t)
                .into_iter()
                .filter(|id| g.card_defs.get(id).map(|c| c.suit == want).unwrap_or(false))
                .collect();
            let next_data = json!({ "step": "judge", "card": card, "targets": targets, "idx": idx + 1 });
            match take_answer(g, "tieqi") {
                None => {
                    let mut b = PendingBuilder::new(t, "pick_own").skill("tieqi");
                    for id in &cands {
                        b = b.option(format!("card:{id}"), format!("弃置【{}】抵消铁骑", card_label(g, id)));
                    }
                    let data = json!({ "step": "judged", "card": card, "target": t, "idx": idx, "targets": targets, "suit": suit });
                    Ok(Step::NeedWith(
                        Stage::PickOwn,
                        data.clone(),
                        b.escape("不弃置（不能使用【闪】）").data(data).build(),
                    ))
                }
                Some(a) if a == "pass" => {
                    // 逐目标「不能使用【闪】」。
                    g.use_opts_mut(&card).unresponsive.push(t);
                    Ok(Step::Next(Stage::Choose, next_data))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("铁骑：只能弃置同花色的手牌/装备".into());
                    }
                    flow::discard(g, t, &id, "铁骑");
                    Ok(Step::Next(Stage::Choose, next_data))
                }
                Some(o) => Err(format!("铁骑：未知选项 {o}")),
            }
        }"""),
])

# ---------------- b9：观星任意顺序（选牌 + 顶/底） ----------------
patch('src/kernel/skills/b9.rs', [
    ("""    // place：逐张置顶 / 置底。
    let mut cards = common::cards_of(ask, "cards");
    let idx = ask.data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let mut all_bottom = ask.data.get("all_bottom").and_then(|v| v.as_bool()).unwrap_or(true);
    if idx >= cards.len() {
        if end_phase {
            g.set_counter(&turn_key(actor, "guanxing_again"), 0);
        } else if all_bottom {
            g.set_counter(&turn_key(actor, "guanxing_again"), 1);
        }
        return Ok(Step::Finish("观星：结算完成".into()));
    }
    let card = cards[idx].clone();
    match take_answer(g, "guanxing") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("guanxing");
            b = b.option("top", format!("将【{}】置于牌堆顶", card_label(g, &card)));
            b = b.option("bottom", format!("将【{}】置于牌堆底", card_label(g, &card)));
            Ok(Step::Need(
                b.escape_mandatory("（必选）")
                    .data(json!({ "step": "place", "cards": cards, "idx": idx, "all_bottom": all_bottom }))
                    .build(),
            ))
        }
        Some(a) if a == "top" => {
            g.deck.push(card.clone());
            all_bottom = false;
            cards.retain(|c| *c != card);
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "place", "cards": cards, "idx": idx, "all_bottom": all_bottom }),
            ))
        }
        Some(a) if a == "bottom" => {
            g.deck.insert(0, card.clone());
            cards.retain(|c| *c != card);
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "place", "cards": cards, "idx": idx, "all_bottom": all_bottom }),
            ))
        }
        Some(o) => Err(format!("观星：未知选项 {o}")),
    }
}""",
     """    // place：**任意顺序**——每次任选一张剩余牌，再决定置顶/置底。
    let mut cards = common::cards_of(ask, "cards");
    let mut all_bottom = ask.data.get("all_bottom").and_then(|v| v.as_bool()).unwrap_or(true);
    if cards.is_empty() {
        if end_phase {
            g.set_counter(&turn_key(actor, "guanxing_again"), 0);
        } else if all_bottom {
            g.set_counter(&turn_key(actor, "guanxing_again"), 1);
        }
        return Ok(Step::Finish("观星：结算完成".into()));
    }
    match take_answer(g, "guanxing") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("guanxing");
            for id in &cards {
                b = b.option(format!("top:{id}"), format!("【{}】置于牌堆顶", card_label(g, id)));
                b = b.option(format!("bottom:{id}"), format!("【{}】置于牌堆底", card_label(g, id)));
            }
            let data = json!({ "step": "place", "cards": cards, "all_bottom": all_bottom });
            Ok(Step::NeedWith(
                Stage::Choose,
                data.clone(),
                b.escape_mandatory("（必选）").data(data).build(),
            ))
        }
        Some(a) if a.starts_with("top:") => {
            let id = a[4..].to_string();
            if !cards.iter().any(|c| *c == id) {
                return Err("观星：牌不在观星区".into());
            }
            g.deck.push(id.clone());
            all_bottom = false;
            cards.retain(|c| *c != id);
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "place", "cards": cards, "all_bottom": all_bottom }),
            ))
        }
        Some(a) if a.starts_with("bottom:") => {
            let id = a[7..].to_string();
            if !cards.iter().any(|c| *c == id) {
                return Err("观星：牌不在观星区".into());
            }
            g.deck.insert(0, id.clone());
            cards.retain(|c| *c != id);
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "place", "cards": cards, "all_bottom": all_bottom }),
            ))
        }
        Some(o) => Err(format!("观星：未知选项 {o}")),
    }
}"""),
])

# ---------------- b8：问卦交牌事件 ----------------
patch('src/kernel/skills/b8.rs', [
    ("""                Some(a) if a.starts_with("card:") => {
                    g.cards.insert(card.clone(), Zone::Draw);
                    if a == "opt1" {""",
     """                Some(a) if a.starts_with("card:") => {
                    g.cards.insert(card.clone(), Zone::Draw);
                    if a == "opt1" {"""),
    ("""                    let id = a[5..].to_string();
                    if !hand.iter().any(|c| *c == id) {
                        return Err("问卦：只能交出自己的手牌".into());
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "place", "owner": owner, "card": id }),
                    ))""",
     """                    let id = a[5..].to_string();
                    if !hand.iter().any(|c| *c == id) {
                        return Err("问卦：只能交出自己的手牌".into());
                    }
                    // 交牌事实（交给徐氏 → 随后置于牌堆）。
                    let mut lost = super::super::event::Event::card(
                        EventKind::CardLost,
                        Some(actor),
                        id.clone(),
                        Some("问卦".to_string()),
                    );
                    lost.by = Some(actor);
                    lost.from = Some(actor);
                    let e = lost.snapshot(g);
                    g.push_event(e);
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "place", "owner": owner, "card": id }),
                    ))"""),
])
print("ok")
