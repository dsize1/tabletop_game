def patch(path, pairs):
    s=open(path,encoding='utf-8').read()
    for old,new in pairs:
        if old not in s:
            raise SystemExit("MISSING in %s:\n%s" % (path, old[:400]))
        s=s.replace(old,new,1)
    open(path,'w',encoding='utf-8').write(s)

# ---------- skills.rs: b9 wiring + 新单出口 ----------
patch('src/kernel/skills.rs', [
("pub mod b8;","pub mod b8;\npub mod b9;"),
("        .chain(b8::SKILLS.iter())","        .chain(b8::SKILLS.iter())\n        .chain(b9::SKILLS.iter())"),
('''    "zhuandui", "tianbian", "funan", "jiexun", "bizhuan", "tongbo",
];''','''    "zhuandui", "tianbian", "funan", "jiexun", "bizhuan", "tongbo",
    // B9 批次（界标·蜀：7 将 / 14 技能，2026-11-16 收口）
    "rende", "jijiang", "yijue", "wusheng", "tishen", "paoxiao", "yajiao", "longdan",
    "mashu", "tieqi", "kongcheng", "guanxing", "qicai",
];'''),
('''        .or_else(|| b8::handler(code))
}''','''        .or_else(|| b8::handler(code))
        .or_else(|| b9::handler(code))
}'''),
('''        .or_else(|| b8::availability(code))
}''','''        .or_else(|| b8::availability(code))
        .or_else(|| b9::availability(code))
}'''),
# 非锁定技失效（义绝/铁骑）
('''pub fn active(g: &Game, seat: usize, code: &str) -> bool {
    if owned(g, seat, code) {
        return true;
    }''','''pub fn active(g: &Game, seat: usize, code: &str) -> bool {
    // 义绝/铁骑：「本回合非锁定技失效」——锁定技与装备特效不受影响。
    if g.counter(&format!("suppress.{seat}.{}", g.turn)) > 0 {
        if let Some(d) = def(code) {
            if !matches!(d.kind, SkillKind::Locked | SkillKind::Equip) {
                return false;
            }
        }
    }
    if owned(g, seat, code) {
        return true;
    }'''),
# 牌面替代：b9（武圣/龙胆）
('''pub fn use_alias(g: &Game, seat: usize, id: &str) -> Option<&'static str> {
    b6::use_alias(g, seat, id)
}''','''pub fn use_alias(g: &Game, seat: usize, id: &str) -> Option<&'static str> {
    b6::use_alias(g, seat, id).or_else(|| b9::use_alias(g, seat, id))
}'''),
('''    match b6::response_as(g, seat, id, kind) {
        CardAs::As(code) => return Some(code),
        CardAs::Deny => return None,
        CardAs::NoOpinion => {}
    }''','''    for opinion in [
        b6::response_as(g, seat, id, kind),
        b9::response_as(g, seat, id, kind),
    ] {
        match opinion {
            CardAs::As(code) => return Some(code),
            CardAs::Deny => return None,
            CardAs::NoOpinion => {}
        }
    }'''),
# 使用起步：b9（武圣方片杀无距离）
('''pub fn on_use_start(g: &mut Game, actor: usize, card: &str) {
    b8::on_use_start(g, actor, card);
}''','''pub fn on_use_start(g: &mut Game, actor: usize, card: &str) {
    b8::on_use_start(g, actor, card);
    b9::on_use_start(g, actor, card);
}

/// 【杀】造成伤害时的技能加成（义绝红桃杀 / 咆哮抵消后下一杀）。
pub fn sha_damage_bonus(g: &mut Game, actor: usize, target: usize, card: &str) -> i32 {
    b9::sha_damage_bonus(g, actor, target, card)
}

/// 【杀】被抵消后（八卦/技能视为闪也算）的技能回调（咆哮）。
pub fn on_sha_dodged(g: &mut Game, actor: usize) {
    b9::on_sha_dodged(g, actor);
}

/// 代出【杀】的接力完成（激将：代出者回合外可令主公摸一张）。
pub fn on_relay_sha(g: &mut Game, actor: usize, source: usize) {
    b9::on_relay_sha(g, actor, source);
}

/// 锦囊无距离限制（奇才）。
pub fn trick_no_distance(g: &Game, actor: usize) -> bool {
    b9::trick_no_distance(g, actor)
}

/// 防止弃置（奇才：他人弃置你的防具/宝物）。
pub fn prevent_discard(g: &Game, by: usize, owner: usize, id: &str) -> bool {
    b9::prevent_discard(g, by, owner, id)
}'''),
('''pub fn deny_target_card(g: &Game, user: usize, target: usize, code: &str) -> bool {
    b2::deny_target_card(g, user, target, code) || b8::deny_target_card(g, user, target, code)
}''','''pub fn deny_target_card(g: &Game, user: usize, target: usize, code: &str) -> bool {
    b2::deny_target_card(g, user, target, code)
        || b8::deny_target_card(g, user, target, code)
        || b9::deny_target_card(g, user, target, code)
}'''),
])

# ---------- card_play.rs ----------
patch('src/kernel/card_play.rs', [
# 铁骑等目标级技能：在 b8 的 fan-out 里追加（同一处注册）
('''    for (owner, role) in owners {
        super::super::resolve::push_ask(
            g,
            "zhuandui",''','''    // 铁骑：使用者侧（指定目标后）询问。
    if g.seats.get(user).map(|s| s.alive).unwrap_or(false) && skills::active(g, user, "tieqi") {
        super::super::resolve::push_ask(
            g,
            "tieqi",
            Stage::Choose,
            user,
            json!({ "step": "sha_targeted", "role": "user", "user": user, "targets": targets, "card": card }),
        );
        pushed = true;
    }
    for (owner, role) in owners {
        super::super::resolve::push_ask(
            g,
            "zhuandui",'''),
# 【杀】伤害加成（义绝/咆哮）：命中分支
('''            let amount = 1 + jiu + if guding { 1 } else { 0 };''','''            let amount = 1 + jiu + if guding { 1 } else { 0 }
                + super::skills::sha_damage_bonus(g, actor, target, card);'''),
# 【杀】被【闪】抵消：咆哮回调
('''        "blocked" => {
            // 被闪抵消：青龙偃月刀（再杀）/ 贯石斧（弃两张牌强命中）。''','''        "blocked" => {
            // 被闪抵消（含八卦/技能视为闪）：咆哮回调。
            super::skills::on_sha_dodged(g, target);
            // 被闪抵消：青龙偃月刀（再杀）/ 贯石斧（弃两张牌强命中）。'''),
# 代出接力：激将摸牌回调
('''    push_virtual_use(g, actor, def_code, origin.to_vec(), targets, false)
}''','''    let vid = push_virtual_use(g, actor, def_code, origin.to_vec(), targets, false)?;
    super::skills::on_relay_sha(g, actor, source_seat);
    Ok(vid)
}'''),
# 奇才/锦囊无距离：顺手/兵粮的距离检查
('''            Effect::Snatch => {
                if super::gear::distance(g, actor, s) > 1 {
                    continue;
                }''','''            Effect::Snatch => {
                if !super::skills::trick_no_distance(g, actor)
                    && super::gear::distance(g, actor, s) > 1
                {
                    continue;
                }'''),
('''                if def.effect == Effect::SupplyShortage
                    && super::gear::distance(g, actor, s) > 1
                {
                    continue;
                }''','''                if def.effect == Effect::SupplyShortage
                    && !super::skills::trick_no_distance(g, actor)
                    && super::gear::distance(g, actor, s) > 1
                {
                    continue;
                }'''),
])

# ---------- flow.rs: 奇才防止弃置 ----------
patch('src/kernel/flow.rs', [
('''    let zone = g.cards.get(id)?.clone();
    let owner = zone.seat()?;
    if !matches!(zone, super::Zone::Hand(_) | super::Zone::Equip(..)) {
        return None;
    }''','''    let zone = g.cards.get(id)?.clone();
    let owner = zone.seat()?;
    if !matches!(zone, super::Zone::Hand(_) | super::Zone::Equip(..)) {
        return None;
    }
    // 奇才：防止**其他角色**弃置你的防具/宝物（「获得」不受影响）。
    if super::skills::prevent_discard(g, by, owner, id) {
        return None;
    }'''),
])

# ---------- b2 集智升级为界集智 ----------
patch('src/kernel/skills/b2.rs', [
('''    // —— 临时技（决堰·宝物：本回合获得「集智」）——
    SkillDef {
        code: "jizhi",
        name: "集智",
        kind: SkillKind::Normal,
        grant_only: true,
        transform: false,
        faces: &[Face {
            window: Window::OwnTurn,''','''    // —— 集智（界黄月英自带；决堰·宝物也能授予）——
    SkillDef {
        code: "jizhi",
        name: "集智",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,'''),
('''                super::super::Slot::Treasure => {
                    g.set_counter(&turn_key(actor, "jizhi"), 1);
                }''','''                super::super::Slot::Treasure => {
                    // 宝物栏：本局获得「集智」（授予统一入口；计数保留给旧用例）。
                    crate::kernel::skills::grant(g, actor, "jizhi");
                    g.set_counter(&turn_key(actor, "jizhi"), 1);
                }'''),
])

# b2::jizhi 界版实现：非转化锦囊 + 摸到基本牌可弃置换手牌上限
patch('src/kernel/skills/b2.rs', [
('''fn jizhi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if g.counter(&turn_key(actor, "jizhi")) == 0 {
        return Ok(Step::Done);
    }
    let Some(card) = card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let Some(def) = g.card_defs.get(&card).and_then(|c| cards::def(&c.def)) else {
        return Ok(Step::Done);
    };
    if def.category != cards::Category::Trick {
        return Ok(Step::Done);
    }
    match take_answer(g, "jizhi") {
        None => Ok(trigger_ask(
            actor,
            "jizhi",
            "集智：摸一张牌",
            json!({ "step": "ask", "card": card }),
        )),
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            flow::draw(g, actor, 1);
            Ok(Step::Finish("集智：摸一张".into()))
        }
        Some(o) => Err(format!("集智：未知选项 {o}")),
    }
}''','''fn jizhi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !crate::kernel::skills::active(g, actor, "jizhi") {
        return Ok(Step::Done);
    }
    let Some(card) = card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let Some(def) = g.card_defs.get(&card).and_then(|c| cards::def(&c.def)) else {
        return Ok(Step::Done);
    };
    if def.category != cards::Category::Trick {
        return Ok(Step::Done);
    }
    // 界集智：只对**非转化**锦囊生效（`cause` 区分真实使用 / 转化 / 视为）。
    let cause = ask.data.get("cause").and_then(|v| v.as_str()).unwrap_or("使用");
    if cause != "使用" {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "hand_limit" {
        let drawn = ask.data.get("drawn").and_then(|v| v.as_str()).unwrap_or("").to_string();
        match take_answer(g, "jizhi") {
            None => {
                let mut b = super::super::pending::PendingBuilder::new(actor, "choose").skill("jizhi");
                b = b.option("discard", format!("弃置【{}】，本回合手牌上限 +1", card_label(g, &drawn)));
                Ok(Step::Need(
                    b.escape("保留该牌")
                        .data(json!({ "step": "hand_limit", "drawn": drawn }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Finish("集智：保留摸到的牌".into())),
            Some(a) if a == "discard" => {
                flow::discard(g, actor, &drawn, "集智");
                g.bump(&turn_key(actor, "hand_limit_extra"));
                Ok(Step::Finish("集智：弃置基本牌，本回合手牌上限 +1".into()))
            }
            Some(o) => Err(format!("集智：未知选项 {o}")),
        }
    } else {
        match take_answer(g, "jizhi") {
            None => Ok(trigger_ask(
                actor,
                "jizhi",
                "集智：摸一张牌",
                json!({ "step": "ask", "card": card }),
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => {
                let drawn = flow::draw(g, actor, 1);
                let basic = drawn
                    .first()
                    .and_then(|id| g.card_defs.get(id))
                    .and_then(|c| cards::def(&c.def))
                    .map(|d| d.category == cards::Category::Basic)
                    .unwrap_or(false);
                if basic {
                    let id = drawn[0].clone();
                    return Ok(Step::Next(
                        super::super::resolve::Stage::Choose,
                        json!({ "step": "hand_limit", "drawn": id }),
                    ));
                }
                Ok(Step::Finish("集智：摸一张".into()))
            }
            Some(o) => Err(format!("集智：未知选项 {o}")),
        }
    }
}'''),
])
print("ok")
