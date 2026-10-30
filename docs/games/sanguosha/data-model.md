# 三国杀数据模型（道具中台 + 对局状态）草案

> 状态：**已评审 v0.2**（2026-10-25 用户反馈已并入）；建表前按 §7 最终确认。
> 关联：`docs/games/sanguosha/plan.md`（原始立项需求）、`docs/games/sanguosha/development-plan.md`（阶段计划）、`memory_palace/decisions/ADR-0017-props-admin-data-model.md`。
> 约定来源：`docs/conventions/persistence.md`（SQLite 真相源 / 单写者 / 事件溯源）、`docs/conventions/coding-standards.md`。

## 1. 设计原则

1. **中台只管「元数据 + 素材」，不管「规则」**：名称、描述、图片、花色点数、距离、血量、技能类型可配置；合法性/结算/时机由 Rust 引擎代码决定（ADR-0016）。
2. **运行时状态不拆实体表**：沿用既有 `game_state`（JSON 快照）+ `game_events`（append-only 事件）事件溯源（ADR-0004/0008）；三国杀的状态结构写在 `data` JSON 里，带 `schema_version`。
3. **单写者**：所有写路径收敛到 `crates/server`（REST `/internal/*`，管理员白名单）；中台 SPA 不直连 SQLite。
4. **通用表 + 游戏专用表**：包/素材是跨游戏通用表（`prop_*`）；身份/牌/武将/技能是三国杀专用表（`sgs_*`），约束写在 SQL 里，便于后台表单直接校验。
5. **图片字段先留空**：P1 只建 `image_asset_id`（可空）与 `prop_assets` 表，不做上传；后续接 `apps/scripts/board-assets.mjs` 流水线（webp + 指纹命名）。**素材自行制作，不使用官方图**。
6. **统一生命周期**：所有身份/牌/武将/技能行都有 `status`（新建/测试/审核/上线/下线），下线为软删；引擎与只读接口只消费 `live`。
7. **视图三分区**：公共局面区 / 玩家个人资源区 / 特殊交互区（观星、五谷丰登等），见 §4.2（ADR-0020）。
8. **时间戳**：一律 ISO 8601 UTC 文本（`created_at` / `updated_at`），与现有表一致。

## 2. 表清单

| 表 | 用途 | 阶段 |
|---|---|---|
| `prop_packs` | 道具包（标准 / 风林火山 / 一将成名 / 界限突破 / 牌堆包） | P1 |
| `prop_assets` | 素材（图片）登记；可空引用 | P1 建表，后续启用 |
| `sgs_identities` | 身份定义（主公/忠臣/反贼/内奸） | P1 |
| `sgs_identity_setup` | 人数 → 身份配比（标准 + 双内变体） | P1 |
| `sgs_card_defs` | 牌的定义（名称/分类/距离/目标类型/描述） | P1 |
| `sgs_cards` | 物理牌（花色/点数/所属包/图片覆盖） | P1 |
| `sgs_generals` | 武将（势力/性别/血量/描述/图片） | P1 |
| `sgs_skills` | 技能元数据（类型/描述） | P1 |
| `sgs_general_skills` | 武将 ↔ 技能 | P1 |
| `prop_status_log` | 道具/武将生命周期流转与审核记录 | P1 |
| `game_state_templates` | 局面/资源模板（工作台预览保存与复制） | P5 |
| `game_decision_log` | AI/机器人决策日志（可采样，非真相源） | P6 |
| `sgs_flow_designs` | 武将流程设计 ↔ n8n 工作流绑定 | P5 |
| `game_state` / `game_events` / `player_actions` | 既有表，直接复用 | 已存在 |

## 3. DDL 草案

> 方言：SQLite。`CHECK` 用文本枚举（与现有表风格一致）；迁移走 `crates/storage` 的幂等 `CREATE TABLE IF NOT EXISTS` + 加列守卫。

### 3.1 通用：道具包与素材

```sql
CREATE TABLE IF NOT EXISTS prop_packs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id     TEXT    NOT NULL,              -- 'sanguosha'
    kind        TEXT    NOT NULL,              -- 'general' | 'card' | 'identity'
    code        TEXT    NOT NULL,              -- 'standard' | 'wind' | 'forest' | ...
    name        TEXT    NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL,
    UNIQUE (game_id, kind, code)
);

CREATE TABLE IF NOT EXISTS prop_assets (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id     TEXT    NOT NULL,
    kind        TEXT    NOT NULL,              -- 'card' | 'general' | 'identity' | 'board'
    path        TEXT    NOT NULL,              -- 仓库内相对路径 / 或对象存储 key
    mime        TEXT    NOT NULL,
    width       INTEGER,
    height      INTEGER,
    bytes       INTEGER,
    sha256      TEXT,
    created_at  TEXT    NOT NULL,
    UNIQUE (game_id, path)
);
```

说明：`prop_assets.path` 不存外部 URL，避免素材外链失效；展示时由 server 拼接前缀。

### 3.2 身份

```sql
CREATE TABLE IF NOT EXISTS sgs_identities (
    code            TEXT PRIMARY KEY,           -- 'lord' | 'loyalist' | 'rebel' | 'spy'
    name            TEXT NOT NULL,              -- 主公 / 忠臣 / 反贼 / 内奸
    team            TEXT NOT NULL CHECK (team IN ('lord','rebel','spy')),
    win_condition   TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    image_asset_id  INTEGER REFERENCES prop_assets(id),
    sort_order      INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft','testing','review','live','offline')),
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sgs_identity_setup (
    player_count    INTEGER NOT NULL CHECK (player_count BETWEEN 5 AND 8),
    variant         TEXT    NOT NULL DEFAULT 'default'
                    CHECK (variant IN ('default','double_spy')),
    identity_code   TEXT    NOT NULL REFERENCES sgs_identities(code),
    count           INTEGER NOT NULL CHECK (count >= 0),
    PRIMARY KEY (player_count, variant, identity_code)
);
```

种子数据（按立项文档整理，**已确认 2026-10-25**）：

| 人数 | 变体 | 主 | 忠 | 反 | 内 | 合计 |
|---|---|---|---|---|---|---|
| 5 | default | 1 | 1 | 2 | 1 | 5 |
| 6 | default | 1 | 1 | 3 | 1 | 6 |
| 7 | default | 1 | 2 | 3 | 1 | 7 |
| 8 | default | 1 | 2 | 4 | 1 | 8 |
| 8 | double_spy | 1 | 1 | 4 | 2 | 8 |

- 身份描述初始化：主公（唯一，身份公开）、忠臣（护主、清反内）、反贼（杀主公）、内奸（最后存活，须最后杀主公；双内不结盟）——文本进 P1 seed。
- 血条牌**不入中台**：由武将 `max_hp` / `initial_hp` 在引擎侧生成。

### 3.3 牌（定义 + 物理牌）

```sql
CREATE TABLE IF NOT EXISTS sgs_card_defs (
    code            TEXT PRIMARY KEY,           -- 'sha' | 'shan' | 'tao' | 'jiu' | 'qinggang_sword' | ...
    name            TEXT NOT NULL,
    category        TEXT NOT NULL CHECK (category IN
                      ('basic','weapon','armor','horse','treasure','trick','delayed_trick')),
    description     TEXT NOT NULL DEFAULT '',
    image_asset_id  INTEGER REFERENCES prop_assets(id),
    -- 武器=攻击范围 N；马=距离修正（进攻马 +1 攻击距离 / 防御马 +1 防御距离，方向由 range_type 区分）；锦囊/延时锦囊=距离限制（NULL 不限）
    range_value     INTEGER,
    -- 锦囊 / 延时锦囊：single=单目标；aoe=群体；NULL=非锦囊
    target_mode     TEXT CHECK (target_mode IN ('single','aoe')),
    -- 装备槽位：玩家装备区的显式类型（武器/护具/进攻马/防御马）；非装备=NULL
    equip_slot      TEXT CHECK (equip_slot IN
                      ('weapon','armor','horse_offense','horse_defense','treasure')),
    -- 攻防马类型（仅 category='horse'）：attack=进攻马（+1 攻击距离），defense=防御马（+1 防御距离）
    range_type      TEXT CHECK (range_type IN ('attack','defense')),
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft','testing','review','live','offline')),
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    CHECK ((category IN ('weapon','armor','horse','treasure')) = (equip_slot IS NOT NULL)),
    CHECK ((category = 'horse') = (range_type IS NOT NULL)),
    CHECK (range_type IS NULL OR
           (range_type = 'attack'  AND equip_slot = 'horse_offense' AND range_value = 1) OR
           (range_type = 'defense' AND equip_slot = 'horse_defense' AND range_value = 1))
);

CREATE TABLE IF NOT EXISTS sgs_cards (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    pack_id         INTEGER NOT NULL REFERENCES prop_packs(id),
    def_code        TEXT    NOT NULL REFERENCES sgs_card_defs(code),
    suit            TEXT CHECK (suit IN ('spade','heart','club','diamond')),  -- 初始为空
    rank            INTEGER CHECK (rank BETWEEN 1 AND 13),                    -- A=1 … K=13；初始为空
    image_asset_id  INTEGER REFERENCES prop_assets(id),                       -- 覆盖定义图（可空）
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft','testing','review','live','offline')),
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- 空花色/点数允许多行（初始化为空）；填齐后同一包里不允许重复
CREATE UNIQUE INDEX IF NOT EXISTS idx_sgs_cards_unique_full
    ON sgs_cards (pack_id, def_code, suit, rank)
    WHERE suit IS NOT NULL AND rank IS NOT NULL;
```

要点：

- `sgs_card_defs` 只描述「这类牌是什么」（规则效果、分类、距离、单/AOE）；
- `sgs_cards` 是**牌堆里的每一张物理牌**（花色 + 点数），满足「所有道具手牌都有花色与 A–K 点数，初始化为空」；
- 装备：`category='weapon'` + `range_value`；`category='horse'` + `range_value=+1/-1`；护具用 `category='armor'`；特殊装备（木牛流马）用 `category='treasure'` / `equip_slot='treasure'`（预留）；
- **装备槽位显式化**：`equip_slot` 决定玩家装备区落到哪个槽（`weapon/armor/horse_offense/horse_defense/treasure`），便于距离与替换判定；进攻马 `range_value=+1`、防御马 `range_value=+1`（方向由 `range_type` 区分，见 `rules.md` §7）；`treasure` 为特殊区（木牛流马）预留；
- **攻防马类型显式化**：`range_type=attack|defense` 与 `equip_slot`/`range_value` 由 CHECK 约束一致（用户 2026-10-25 要求「距离值加攻防马类型」）；
- `category` 与 `equip_slot` 必须一致（DDL 末尾 CHECK）：装备类必须有槽位，非装备必须无槽位；
- 延时锦囊用 `category='delayed_trick'`，与普通锦囊同表，靠分类区分。

### 3.4 武将与技能

```sql
CREATE TABLE IF NOT EXISTS sgs_generals (
    code            TEXT PRIMARY KEY,           -- 'liubei' | 'caocao' | ...
    external_id     TEXT UNIQUE,                -- 'sanguosha.com:hero:1'（官网 gid，导入幂等键）
    name            TEXT NOT NULL,
    kingdom         TEXT NOT NULL CHECK (kingdom IN ('wei','shu','wu','qun','jin','god')),
    gender          TEXT NOT NULL DEFAULT 'unknown'
                    CHECK (gender IN ('male','female','unknown')),
    pack_id         INTEGER NOT NULL REFERENCES prop_packs(id),
    max_hp          REAL NOT NULL CHECK (max_hp > 0 AND max_hp <= 15),
    initial_hp      REAL NOT NULL DEFAULT 0 CHECK (initial_hp >= 0 AND initial_hp <= 15),
    quality         TEXT,                       -- 官方品质（普通/稀有/...），非规则必需
    description     TEXT NOT NULL DEFAULT '',   -- 武将特点
    biography       TEXT NOT NULL DEFAULT '',   -- 武将生平
    image_asset_id  INTEGER REFERENCES prop_assets(id),
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft','testing','review','live','offline')),
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    CHECK (initial_hp = 0 OR initial_hp <= max_hp)
);

CREATE TABLE IF NOT EXISTS sgs_skills (
    code            TEXT PRIMARY KEY,           -- 'jijiang' | 'jianxiong' | ...
    external_id     TEXT UNIQUE,                -- 'sanguosha.com:hero:1#skill:0'（导入幂等键）
    name            TEXT NOT NULL,
    type            TEXT NOT NULL CHECK (type IN
                      ('lord','normal','locked','limited','awakening')),
    description     TEXT NOT NULL DEFAULT '',
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft','testing','review','live','offline')),
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sgs_general_skills (
    general_code    TEXT    NOT NULL REFERENCES sgs_generals(code),
    skill_code      TEXT    NOT NULL REFERENCES sgs_skills(code),
    sort_order      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (general_code, skill_code)
);

CREATE INDEX IF NOT EXISTS idx_sgs_skills_name ON sgs_skills (name);
CREATE INDEX IF NOT EXISTS idx_sgs_generals_pack ON sgs_generals (pack_id);
```

要点：

- 技能类型只存**元数据**；技能的实际时机/结算由引擎 `Skill` 实现（ADR-0016）。中台**不写入**触发逻辑，避免「配置即规则」导致无法调试。
- 武将包枚举（标准/风/火/林/山/一将成名/界限突破/…）由 `prop_packs` 数据行表达，不写死 CHECK，方便后续扩展。
- `max_hp` / `initial_hp` 区分「体力上限」与「初始体力」（主公 +1 体力由身份规则在引擎内处理，不改中台数据）。
- `max_hp` 用 REAL 以容纳国战半体力（1.5/2）；**身份局导入时排除 `国战` 包**，只取整数体力。
- `initial_hp = 0` 表示「未提供」，导入/引擎按 `initial_hp = max_hp` 处理；主公 +1 在引擎侧叠加。
- `gender` 官网未提供，导入默认 `unknown`，中台后续补；`external_id` 是官网 gid 的幂等键。

### 3.5 生命周期状态（所有身份/牌/武将/技能）

统一 `status` 枚举（用户 2026-10-25 确认）：

| 值 | 中文 | 含义 |
|---|---|---|
| `draft` | 新建 | 刚录入，仅中台可见 |
| `testing` | 测试 | 可在预览/测试对局使用 |
| `review` | 审核 | 待人工审核上线 |
| `live` | 上线 | 引擎与只读接口可见 |
| `offline` | 下线 | **软删**，不再出现在新对局；历史对局按事件回放不受影响 |

规则：

- 引擎/只读接口只消费 `status='live'`；中台默认展示全部，可按状态筛选。
- 软删不改历史：`game_state` / `game_events` 里保存的是当时的牌/武将快照或 code，可继续回放。
- `prop_packs` 仍用 `enabled` 表示「包是否可用」，与单条道具的生命周期分开。
- 状态迁移：`draft ↔ testing → review → live ↔ offline`；`review` 驳回回 `draft`；`offline` 重新上线需再走 `review`。
- 所有状态迁移写 `prop_status_log`（审计/审核记录）：

```sql
CREATE TABLE IF NOT EXISTS prop_status_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id     TEXT    NOT NULL,
    entity_kind TEXT    NOT NULL CHECK (entity_kind IN
                  ('identity','card_def','card','general','skill')),
    entity_code TEXT    NOT NULL,
    from_status TEXT,
    to_status   TEXT    NOT NULL,
    actor       TEXT,                         -- openid / 'import' / 'system'
    note        TEXT,
    created_at  TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_prop_status_log_entity
    ON prop_status_log (game_id, entity_kind, entity_code, created_at);
```

### 3.6 官网抓取导入映射（2026-10-25 预研）

`docs/games/sanguosha/data/heroes.json`（696 武将 / 1383 技能）→ 中台表：

| 源字段 | 目标 | 规则 |
|---|---|---|
| `gid` | `sgs_generals.external_id` | `sanguosha.com:hero:<gid>`，幂等 upsert |
| `name` | `sgs_generals.name` | 去首尾空格；同名不同包用 `pack` 区分 |
| `pack` | `prop_packs` | 中文 → code：标准 `standard`、风林火山 `wind_fire_forest_mountain`、一将成名 `yijiang`、界限突破 `boundary_break`、神将 `god`、谋 `mou`、门阀士族 `clan`、魔 `demon`、国战 `guozhan`、其他 `other` |
| `kingdom` / `kingdom_code` | `sgs_generals.kingdom` | 魏/蜀/吴/群/晋/神 → `wei/shu/wu/qun/jin/god` |
| `hp` | `sgs_generals.max_hp` | 国战半体力（1.5）不导入身份局 |
| `initial_hp` | `sgs_generals.initial_hp` | 0 → 存 0，引擎按 = max_hp 处理 |
| `quality` | `sgs_generals.quality` | 原样 |
| `characteristic` | `sgs_generals.description` | 原样 |
| `biography` | `sgs_generals.biography` | 原样 |
| `avatar` / `cover` | **不导入** | 不使用官方图片，素材后续自制 |
| `skills[].name` | `sgs_skills.name` | 按名称去重（1054 个唯一名） |
| `skills[].description` | `sgs_skills.description` | 1 条为空（gid 639 秦朗），跳过该空技能 |
| `skills[].type_guess` | `sgs_skills.type` | `lord/locked/limited/awakening/normal`；null → `normal`，**需人工复核** |
| `skills[].external_id` | `sgs_skills.external_id` | `sanguosha.com:hero:<gid>#skill:<idx>` |
| `gender` | `sgs_generals.gender` | 官网缺失 → `unknown`，中台补 |

- 技能 `code` 由导入脚本生成（如 `sk_<名称哈希前10位>`），跨武将同名共享一行；人工可在中台改 code。
- 牌表：官网**没有公开的结构化牌表/接口**（sitemap 仅 `/hero/*` 等页面）；已从社区开源牌堆整理 `cards.json`（标准 108 + 军争 52）与 `cards.md`，**P1 人工逐张复核后入库**；若后续找到官方数据再替换。

### 3.7 引擎内置数据（生成脚本，P4.0 已落地）

中台「元数据只读 API」留 P5：P4 阶段引擎消费**内置生成数据**，来源与中台同一批 `data/*.json`，保证口径一致。

| 生成产物（`crates/engines/sanguosha/src/data.rs`） | 说明 |
|---|---|
| `CARD_DEFS` | 牌定义（按 code 去重；跨包同名定义字段必须一致） |
| `STANDARD_DECK` / `JUNZHENG_DECK` | 物理牌表（108 / 52），`PhysicalCard.pack` 标注所属牌堆包 |
| `STANDARD_DECK_SIZE` / `JUNZHENG_DECK_SIZE` / `FULL_DECK_SIZE` | 108 / 52 / 160 |
| `DECK_PACKS` | 支持的牌堆包（顺序即建堆顺序：`standard` → `junzheng`） |
| `deck_cards(&packs)` | 按 `Config.packs` 组堆；未知包/空组合报错；同一牌堆内 `pack` 必须与牌堆名一致 |
| `STANDARD_GENERALS` / `WIND_FIRE_FOREST_MOUNTAIN_GENERALS` | 武将（标准 25 / 风林火山 48；`GeneralDef.pack` 为武将包 code） |
| `GENERAL_PACK_KEYS` / `GENERAL_PACKS` | 武将包 key 表（`standard` → `wind_fire_forest_mountain`）与按包拼接的静态数据 |
| `general_codes_in_packs(&packs)` | 按 `Config.general_packs` 过滤武将码（选将池）；未实现武将再由 `skills::general_ready` 过滤 |

- 生成：`npm run gen:sgs-engine-data`；门禁：`npm run check:sgs-engine-data`（`--check` 逐字节比对）。
- 默认生成标准 25 + 风林火山 48 = **73 将**（`DEFAULT_GENERAL_PACKS`）；`--general-packs` 可裁剪（新增包需在脚本 `GENERAL_PACKS` 登记 gids 与性别表）。
- 选将池：`Config.general_packs`（默认 `["standard"]`，旧快照自动补齐）→ 包过滤 → `skills::general_ready`（技能全部 implemented）→ 候选。风林火山当前只实现风包 8 将，其余 40 位不进池。
- 牌堆口径：默认 `["standard","junzheng"]` = **160 张**，可在 lobby 切到 `standard` = 108 张（`lobby:set_packs`）。
- 武将包口径：默认 `["standard"]`，可在 lobby 切到 `["standard","wind_fire_forest_mountain"]`（`lobby:set_general_packs`；风林火山 48 位数据已生成，未实现武将不进池）。
- 旧快照兼容：schema v2 及以前没有 `config.packs` / `config.general_packs`，反序列化分别按默认 `["standard","junzheng"]` / `["standard"]` 补齐（**不重建已有牌堆**，实例仍以快照中的 `cards` 为准）。

## 4. 运行时状态模型（不建实体表）

复用：

- `game_state (game_id, room_id, state, version, updated_at)`：完整 `GameState` JSON 快照；
- `game_events`：append-only 事件（随机结果必须入事件，见 ADR-0004/0008）；
- `player_actions`：人类可读动作流水（可选，保留）。

### 4.1 `GameState.data` 草案（schema_version 1）

```jsonc
{
  "schema_version": 1,
  "mode": "identity",                 // identity | guozhan（国战未排期，仅预留接缝）
  "phase": "lobby",                   // lobby | deal | turn | dying | settle | finished | aborted
  "host": "openid",
  "config": {                         // 规则配置（默认值，可扩展）
    "lord_candidate_count": 5,        // 主公候选中的非主公武将数
    "other_candidate_count": 5,       // 其他玩家同时获得的候选数
    "max_hand_cards": 20,             // 任何时刻手牌持有上限（弃牌阶段上限仍为当前体力值）
    "base_attack_range": 1,
    "base_defense_range": 1,
    "packs": ["standard", "junzheng"], // P4.0：牌堆包组合（默认 160 张；旧对局按创建时固定）
    "general_packs": ["standard"]      // P4.4：武将包组合（默认仅标准；开启后选将池扩大）
  },
  "player_count": 5,
  "identity_variant": "default",
  "round": 1,
  "turn": {
    "seat": 0,                        // seats 下标
    "phase": "judge",                 // judge | draw | play | discard | end
    "extra_phases": []                // 武将技能插入的额外阶段（如「准备阶段」前后）
  },
  "seats": [
    {
      "player_id": "openid",
      "identity": "lord",             // 主公公开；其余在 public_state 抹除
      "general": "liubei",
      "skills": ["jijiang", "rende"],
      "hp": 4, "max_hp": 4,
      "alive": true,
      "hand": ["c12", "c7"],          // 牌实例 id（顺序即手牌顺序）
      // 键 = sgs_card_defs.equip_slot；玩家装备区按槽位判定
      "equips": { "weapon": null, "armor": null, "horse_offense": null, "horse_defense": null, "treasure": null },
      "judge_zone": [],               // 延时锦囊实例 id（按放入顺序存；末尾=栈顶，LIFO 先判）
      "flags": {},                    // 技能私有标记（如觉醒已触发、限定技已用、蛊惑「缠怨」）
      "marks": {},                    // P4.4：武将牌上的标记（code → 牌实例，如不屈「创」）
      "flipped": false,               // P4.4：翻面（背面朝上：跳过下个回合）
      "chained": false                // P4.3：铁索连环横置
    }
  ],
  // "cards": {
    // 牌实例表：id → 物理牌 + 当前区域 + 归属
    "c12": { "def": "sha", "suit": "spade", "rank": 7, "zone": "hand", "owner": "openid" },
    "c7":  { "def": "shan", "suit": "heart", "rank": 2, "zone": "hand", "owner": "openid" }
  },
  "deck": { "draw": ["c1", "c2"], "discard": [], "void": [] },
  "pending": null,                    // { kind, actor, skill?, options[], data }
  "pending_queue": [],                // 同一次结算产生的多个交互排队
  "skill_subscriptions": [
    { "event": "damage_after", "skill": "jianxiong", "owner": "openid", "priority": 0 }
  ],
  // 特殊交互区（技能/阶段私有，按需出现；视图投影见 §4.2）
  "special_zones": {
    // 观星：2 列 × 5 槽固定；L1..L5 = 牌堆顶侧，R1..R5 = 牌堆底侧；提交时每列跳过空槽上到下压紧，左列接右列
    "guanxing": {
      "owner": "openid", "visibility": "owner", "rows": 5, "cols": 2, "rev": 1,
      "slots": { "L1": "c3", "L2": "c4", "L3": null, "L4": null, "L5": null,
                 "R1": null, "R2": null, "R3": null, "R4": null, "R5": null },
      "skill": "guanxing",
      "interaction": { "kind": "ordered", "commit": "arrange", "slots": "L1..L5,R1..R5" }
    },
    // 五谷丰登（P2.5）：亮出的牌公开；释放者先选，之后按座位左手侧依次
    "wugu": {
      "visibility": "public",
      "cards": ["c5", "c6"],                    // 尚未被选走的亮牌（牌面公开）
      "choosers": ["u1", "u2", "u3", "u4", "u5"], // 选择顺序
      "cursor": 2, "current": "u3",
      "picked": [{ "player": "u1", "card": "c9" }],
      "interaction": { "kind": "one", "commit": "pick" }
    }
  },
  "winner": null                      // { team: "lord"|"rebel"|"spy", players: [...] }
}
```

说明：`equips` 的键就是 `sgs_card_defs.equip_slot`（武器/护具/进攻马/防御马/特殊），玩家装备区按槽位判定；P4.4.1 的「视为装备」（八阵）不写入 `equips`/`cards`，由 `skills::virtual_equip` 在规则与视图投影时临时生效；拼点（驱虎/天义）只通过结算栈 `PindianFrame` + `Outcome::Pindian` 存在，不落持久字段。`special_zones` 是通用占位，具体字段由技能实现定义（观星、五谷丰登、判定展示等），未出现时为空对象。观星的完整字段、交互与回写口径见 `special-zones.md` §5 与 `ui-spec.md` §7（**P2.5 引擎契约已落地**：`arrange:<csv>` 校验 + `settle_slots()` 纯函数）；五谷丰登见 `special-zones.md` §6（P2.5 已实现）。

### 4.2 视图三分区（ADR-0020）

引擎内部保持完整状态；对外投影固定为三个区，**交互只发生在玩家个人资源区**：

| 区 | 内容 | 消费者 |
|---|---|---|
| `public_board` | 公共局面与资源：座位顺序、公开身份、武将/体力/装备/判定区、弃牌堆、牌堆余量、当前轮次/阶段、公共特殊区 | 所有端（Web 公共区；Q 只查摘要） |
| `player_area` | 玩家个人资源：本人手牌明细、本人技能与可用动作、个人 pending、加密选项代码、私有标记 | 仅本人（Web 个人资源区，各端交互基座） |
| `special_zones` | 特殊交互区：观星牌堆顶、五谷丰登候选/选择、判定展示等，按技能声明的可见性下发 | 被授权玩家 / 公共 |

- `get_state` = `public_board` + 公共 `special_zones`；`get_player_view` = `public_board` + 本人 `player_area` + 本人可见的 `special_zones`。
- **schema v2（P2.3）**：`data` 内新增 `resolution`（结算栈）与 `outcome`（子帧结果）；`pending` 是栈顶投影，对外字段不变。判定中的延时锦囊计入 `deck.processing`（牌数守恒）。
- 公共区**只读**；动作只能基于 `player_area`（及授权 `special_zones`）发起。
- 多端（Web / WebView / 未来 APP）复用同一套视图分区；Q 群侧**不播报公共局面**，只做开局设定、轮次/状态查询与牌局链接（见开发计划 P7）。
- 敏感选项仍走一次性代码 + 短 token；token 过期由 `#恢复` 重签。

| 信息 | `public_board` | `player_area` |
|---|---|---|
| 身份 | 仅主公；其余隐藏 | 自己可见 |
| 武将 / 技能 / 体力 / 装备 / 判定区 | 公开 | 公开（本人视角） |
| 手牌 | 仅张数 | 自己的手牌明细 + 加密选项代码 |
| 牌堆顺序 / 摸牌结果 | 隐藏 | 与摸牌相关的技能按规则下发 |
| 弃牌堆 / 已亮明牌 | 公开 | 公开 |
| 特殊区 | 按技能可见性 | 本人可见部分 |

## 5. 工作台 / AI 相关表

```sql
-- 局面与资源模板：预览页保存/复制（P5）
CREATE TABLE IF NOT EXISTS game_state_templates (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id     TEXT    NOT NULL,
    name        TEXT    NOT NULL,
    source      TEXT    NOT NULL,             -- 'manual' | 'replay:<room_id>@<seq>'
    snapshot    TEXT    NOT NULL,             -- GameState JSON
    meta        TEXT,                         -- 牌堆/手牌/装备等可复用资源 JSON
    created_by  TEXT,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);

-- AI / 机器人决策日志（可采样丢弃；非真相源，训练数据来源，P6）
CREATE TABLE IF NOT EXISTS game_decision_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id         TEXT    NOT NULL,
    room_id         TEXT    NOT NULL,
    seq             INTEGER NOT NULL,         -- 对应动作落库后的 game_events.seq
    player_id       TEXT    NOT NULL,
    source          TEXT    NOT NULL CHECK (source IN ('human','systemone','bot','random')),
    legal_actions   TEXT    NOT NULL,         -- JSON [{id, description}]
    candidates      TEXT,                     -- 引擎预剪枝候选（≤26）JSON
    chosen          TEXT,
    confidence      REAL,
    latency_ms      INTEGER,
    created_at      TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_game_decision_log_room ON game_decision_log (room_id, seq);

-- 武将流程设计（n8n 绑定，P5）
CREATE TABLE IF NOT EXISTS sgs_flow_designs (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    general_code      TEXT    NOT NULL REFERENCES sgs_generals(code),
    workflow_id       TEXT    NOT NULL,       -- n8n workflow id
    workflow_version  INTEGER,
    status            TEXT    NOT NULL DEFAULT 'draft'
                      CHECK (status IN ('draft','simulating','verified','archived')),
    notes             TEXT,
    created_at        TEXT    NOT NULL,
    updated_at        TEXT    NOT NULL
);
```

## 6. 迁移与索引

- 新增表全部 `CREATE TABLE IF NOT EXISTS`，纳入 `crates/storage/src/lib.rs` 的 `CREATE_TABLES`（或按域拆 `CREATE_SGS_TABLES`），启动幂等执行。
- 需补索引：`sgs_general_skills(skill_code)`、`sgs_cards(def_code)`、`sgs_generals(pack_id)`、`sgs_identity_setup(player_count)`、`game_state_templates(game_id)`。
- 不做破坏性迁移；`sgs_*` 新表与现有表无外键耦合（避免与事件溯源表强绑定）。
- 状态 JSON 的版本演进走 `data.schema_version` + 引擎内迁移函数（与拉斯维加斯 v1→v6 同法）。P4.3/P4.4 新增字段全部 `#[serde(default)]`（`chained`/`marks`/`flipped`/`config.general_packs`），旧快照可直接解析，不 bump schema。
- 牌数守恒（`check_conservation`）统计区域：`draw/discard/processing/void/hands/equips/judge/marks`；武将牌上的标记牌同样计入，死亡时清空进弃牌堆。

## 7. 评审结论与遗留确认

### 7.1 已确认（2026-10-25 用户反馈）

| # | 问题 | 结论 | 落地 |
|---|---|---|---|
| 1 | 牌模型 | **接受「定义表 + 物理牌表」两分**；中台需增加「定义 ↔ 物理牌」关联管理页 | DDL 保持 `sgs_card_defs` / `sgs_cards`；P1 增加关联页（按定义筛选、批量生成物理牌、绑定花色点数） |
| 2 | 花色/点数初始化 | 官网**武将数据已全量抓取**（696 位 / 1383 技能，见 `data/README.md`）；牌表无公开结构化接口，默认人工/社区表 | 武将走 `heroes.json` 导入（§3.6）；牌定义/物理牌 P1 人工录入 |
| 3 | 马的距离 | 用 `range_value` 的 +1/-1，并加**攻防马类型**；玩家道具区需要显式类型便于判定 | 新增 `equip_slot` + `range_type(attack/defense)`，CHECK 约束一致；玩家 `equips` 按槽位存 |
| 4 | 锦囊距离 | `NULL=无距离限制`、`1=距离 1` | 保持 |
| 5 | 技能类型 | 暂定五类：`lord/normal/locked/limited/awakening` | 保持 |
| 6 | 武将/道具生命周期 | 所有道具与武将都加状态：**新建/测试/审核/上线/下线（软删）**；状态迁移需要审核记录 | 所有 `sgs_*` 实体表加 `status`；新增 `prop_status_log`（审批/流转审计） |
| 7 | 决策日志 | 允许采样/关闭 | `game_decision_log` + 采样开关（P6） |
| 8 | 模板归属 | `source` + `created_by` 足够 | 保持 |
| 9 | 特殊区域可见性 | 当前 `owner/public` 够用，**预留其他** | `special_zones.visibility` 预留 `all/team`（未实现）；新增区域必须登记 `special-zones.md` |
| 10 | 规则口径 | 内奸/双内、主杀忠弃牌、响应顺序（含无懈链特殊顺序）、主公体力 +1 | 见 `rules-rulings.md`；观星排序规格已定（`ui-spec.md` §7）；特殊区域见 `special-zones.md` |

### 7.2 其他已确认（影响范围，详见开发计划）

- 牌池**包含军争**（酒、火杀、雷杀等），但**首版只做标准包**跑通整体流程；军争牌定义可在 P1 录入、玩法放后续阶段。
- 图片素材**自行制作**，不使用官方素材。
- **无超时**；长对局可恢复。
- **Q 群侧后置**：原语先行添加；**Web 页提前**且应在设计工作台之前完成；多端通过 WebView 复用；玩家区域三分（公共局面区 / 个人资源区 / 特殊交互区）。
- 游戏**权鉴**放到后续上线阶段再做（当前拉斯维加斯信息公开，权限模型待验证）。
- 死亡奖惩：**杀反贼摸 3 张**、主公杀忠弃全部手牌和装备；**主公先死 → 反贼胜**。
- 选将：主公候选 = 全部主公将 + N 张非主公（默认 5）；其余同时发放 M 张候选（默认 5）；N/M 可配置。
- 手牌：**持有上限 20**（`max_hand_cards`，可配置；超出立即弃至上限）；**弃牌阶段上限仍为当前体力值**（技能可改）。
- 装备区：武器/护具/进攻马/防御马/**特殊区（木牛流马预留）**各 1；后装备顶掉旧装备入弃牌堆。
- 距离：无上限；默认攻击距离 1、防御距离 1；武器 +N、进攻马 +1 攻击距离、防御马 +1 防御距离；顺手/兵粮不含武器范围；技能可叠加。
- 无懈链：从原锦囊使用者左手侧开始，每次有人打出无懈重新从该使用者左手侧开始，允许同一玩家多次响应。

### 7.3 仍需确认 / 验证

1. **牌表来源**：官网无公开结构化牌表（`sitemap` 仅 `/hero/*` 等页面）；按用户裁定 **P1 人工录入**；`cards.json`/`cards.md` 为社区开源牌堆参考（标准 108 + 军争 52），需逐张复核后入库。
2. **武将数据质量**：696 位中 1 条技能描述为空（gid 639 秦朗）、`gender` 全部缺失、54 位国战半体力；导入时按 §3.6 处理，`gender`/`type_guess` **待用户确认**。
3. **版权**：官网文本仅内部开发/初始化使用，上线前确认授权；图片不使用官方素材。
4. **特殊区域**：`all/team` 预留未实现；观星 `ordered` 原语需在 P2 补，五谷丰登/濒死/无懈链待细化（见 `special-zones.md`）。
5. **持有上限超出**：弃置交互由玩家选择还是自动按获得顺序（P2 细化）。
6. **选将**：候选玩家间不重复、未选回池已按用户口径记录；实现细节 P2 确认。
7. **顺手距离例外**：陆逊等武将技能覆盖默认距离（P4 核对）。
8. **权鉴模型**：现有短期 token 是否足够；个人资源区隔离是否要写进 token scope（上线阶段定）。
