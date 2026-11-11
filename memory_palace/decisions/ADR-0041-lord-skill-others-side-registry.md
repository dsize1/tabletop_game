# ADR-0041：主公技「他人侧」注册表（取代 `LORD_SKILLS_USED_BY_OTHERS` 白名单）

- **状态**：Accepted（2026-11-11，用户裁定「主公技不止制霸 —— 其他同势力角色也有该技能」；
  `rules-rulings.md §46`）
- **相关**：`docs/games/sanguosha/rules.md §9`、`docs/games/sanguosha/skills.md`（主公技他人侧表）、
  `align/B14.md`（制霸）
- **结论一句话**：每个主公技在**唯一注册表**（`skills::lord::TABLE`）声明他人侧模式；
  闸门 `skills::lord_gate_ok` 由注册表 + 同势力 + 持有者存活派生，删除硬编码白名单。

## 背景

主公技长期被实现成「持有者的私有技能」：

- `skills/mod.rs` 里 `LORD_SKILLS_USED_BY_OTHERS = ["huangtian"]` 是**唯一**的「他人可用」通道，
  且是硬编码；加技能就要改内核；
- 于是「制霸」的他人挑战（吴势力角色出牌阶段与孙策拼点、孙策可拒绝）一直没有入口：
  `align/B14.md §7` 写了、`§2` 只实现主动那半；
- 黄天能用，但没有任何机制说明「为什么是黄天而不是别的」，也无法校验「新主公技漏了他人侧」。

用户指出这是**一类**问题而不是一个技能的缺口：主公技由**同势力共享**，只是参与方式不同
（他人主动 / 他人触发 / 持有者请求他人响应 / 仅持有者）。

## 决策

1. **注册表即数据**：新增 `skills/lord.rs`：
   ```rust
   pub enum LordSide { OthersActive, OthersTrigger, HolderRequest, HolderOnly }
   pub const TABLE: &[(&str, LordSide)] = &[ ... 13 条 ... ];
   pub fn others_side(code) -> LordSide;   // 未登记 → HolderOnly
   pub fn others_may_act(code) -> bool;    // OthersActive | OthersTrigger
   pub fn holder_of(g, code) -> Option<usize>;   // 真实持有者（不认伪帝借用）
   pub fn shared_with(g, seat, code) -> bool;    // 同势力 + 持有者存活 + h != seat
   ```
   **不改 `SkillDef` 结构**（否则要为 ~296 个 def 机械补字段），表与 `SKILLS` 双向校验。
2. **闸门派生**：`lord_gate_ok(g, owner, code)` = 主公本人/伪帝借用 **或**
   （允许他人侧 ∧ `shared_with`）。`legal_actions`/`execute_active`/`subs` 三处闸门自动统一。
3. **门禁**：`skills::lord::tests::every_lord_skill_declares_others_side` —— 每个 `kind: Lord`
   必须出现在表里，表里不得有非主公技，条数必须相等（漏声明 `cargo test` 红）。
4. **四类归属**（写进 `skills.md` 表）：黄天/制霸 = `OthersActive`；颂威/暴虐/立军/兴衰 = `OthersTrigger`；
   激将/护驾 = `HolderRequest`；救援/勤王/诏缚/血裔/若愚 = `HolderOnly`。
   `OthersTrigger` 的现有实现是「持有者订阅、handler 内判他人条件」，本次**只补声明、不改行为**。
5. **制霸被动挑战落地**（复用注册 code `zhiba`，不新增 code，避免 B14 对齐单「恰好相等」破裂）：
   挑战者路径 + 持有者「接受/拒绝」帧（跨角色 pending，沿用立军/颂威既有模式，不需要 relay 通道）。

## 被否方案

| 方案 | 否决原因 |
|---|---|
| 继续往 `LORD_SKILLS_USED_BY_OTHERS` 里加 `zhiba` | 又一处硬编码；无覆盖校验；别人加入新主公技时仍会漏 |
| 给 `SkillDef` 加 `others_side` 字段 | 要改 ~296 个 `SkillDef` 字面量（机械噪音、diff 巨大），收益与注册表相同 |
| 把制霸拆成新 code（如 `zhiba_challenge`） | 会打破 `check:sgs2-registry` 的「B14 注册表 ≡ 对齐单」精确匹配，且武将技能表里没有这个技能名 |
| 用 relay 通道实现挑战 | 挑战只是「跨角色问答」（目标角色做一个选择），立军/颂威已有现成模式；relay 是 B4 的另一笔欠账 |

## 后果

- 新增主公技必须声明他人侧 → 门禁拦住「沉默实现」；
- 「伪帝（袁术）借用的主公技不开放他人侧」成为显式口径（`holder_of` 只认自带/授予）；
- 黄天的「他人可用」从白名单变成「群势力 + 张角存活且持有」，语义更准确（非群势力不再可能绕过）；
- 遗留：`HolderRequest`（激将/护驾）的内核实现仍是 `relay_played/relay_none` 专用通道（B4 余 3 之一），
  本次只做声明，不迁移。
