# 三国杀抓取数据（官网公开页面）

> 状态：**预研产物（2026-10-25）**。用于道具中台的武将/技能初始化与字段校验。
> 来源：`https://www.sanguosha.com/hero`（列表页内嵌 `const Listdata`）+ `https://www.sanguosha.com/hero/{gid}`（技能详情）。

## 1. 文件

| 文件 | 内容 |
|---|---|
| `heroes.json` | 696 位武将 + 1383 条技能（名称/描述/类型推断），含生平、特点、体力、势力、包、头像/立绘 URL |
| `cards.json` | 标准包 108 张 + 军争 52 张（牌名/花色/点数/分类/距离/目标/伤害类型） |
| `../generals.md` | 由 `heroes.json` 自动生成的武将表（按标准/风林火山等分包、每包一个表格） |
| `../cards.md` | 由 `cards.json` 自动生成的牌表（按标准/军争、基本/锦囊/装备分组） |
| `../../../../apps/scripts/crawl-sanguosha-heroes.mjs` | 抓取脚本（可复现；带缓存、重试、限速） |
| `../../../../apps/scripts/gen-sanguosha-heroes-md.mjs` | `heroes.json` → `generals.md` 生成脚本 |
| `../../../../apps/scripts/gen-sanguosha-cards-md.mjs` | `cards.json` → `cards.md` 生成脚本 |
| `../../../../apps/scripts/gen-sanguosha-engine-data.mjs` | `cards.json` + `heroes.json` → 引擎内置 `crates/engines/sanguosha/src/data.rs`（P4.0：标准 108 + 军争 52 = 160 张 / 标准 25 将；`npm run check:sgs-engine-data` 门禁） |

## 2. 抓取结果（2026-10-25）

- 武将：**696**（列表页 10 个包，按 gid 去重）
- 技能：**1383** 条，去重名称 **1054** 个
- 包分布：一将成名 72、其他 269、国战 110、标准 27、界限突破 87、神将 16、谋 27、门阀士族 34、风林火山 48、魔 6
- 势力：魏/蜀/吴/群/晋/神（官网 `figure`）
- 失败：0

## 3. 字段与限制

| 字段 | 说明 | 限制 |
|---|---|---|
| `gid` | 官网武将 id | 可作为 `sgs_generals.external_id` 的稳定键 |
| `name` / `realname` | 名称 / 带包名 | `realname` 含包名，便于区分同名 |
| `pack` | 包 | 与 `prop_packs.code` 需映射（中文 → code） |
| `kingdom` / `kingdom_code` | 势力 | 含 `神`（`shen`），数据模型 `kingdom` 需加 `god` |
| `hp` / `initial_hp` | 体力 | `initial_hp` 全为 **0**（未提供）→ 导入时按 `initial_hp = hp`；国战武将有 **1.5** 半体力（54 位），身份局导入需排除 `国战` 包 |
| `quality` | 品质 | 官方字段，非规则必需 |
| `biography` / `characteristic` | 生平 / 特点 | 可映射到 `sgs_generals.biography` / `description` |
| `avatar` / `cover` | 官方图 URL | **不使用官方图片**（用户裁定自行制作），仅作参考 |
| `skills[].name` | 技能名 | 同一技能可能跨武将重复（1054 唯一名） |
| `skills[].description` | 技能描述 | 1 条为空：`gid=639 秦朗`（官网原始为空） |
| `skills[].type_guess` | 类型推断 | 由描述前缀/关键词推断（主公技/锁定技/限定技/觉醒技/一般），**需人工复核** |
| `gender` | 性别 | **官网未提供**，导入时留 `unknown`，后续中台补 |

## 4. 版权与使用范围

- 官网文本/图片版权归官方所有；本目录数据**仅用于内部开发与初始化**，上线前需确认授权。
- 图片字段不落库、不用于前端；中台只保留 `image_asset_id`，素材后续自行制作。

## 5. 复现

```bash
# 全量（默认 4 并发、120ms 间隔；缓存到 data/sanguosha-cache/）
node apps/scripts/crawl-sanguosha-heroes.mjs

# 抽样 / 指定输出
node apps/scripts/crawl-sanguosha-heroes.mjs --limit 20 --out .tmp/heroes.test.json

# 忽略缓存重抓
node apps/scripts/crawl-sanguosha-heroes.mjs --refresh

# 生成武将表 docs/games/sanguosha/generals.md
node apps/scripts/gen-sanguosha-heroes-md.mjs

# 生成牌表 docs/games/sanguosha/cards.md
node apps/scripts/gen-sanguosha-cards-md.mjs
```

- 遵守 `robots.txt`（不抓 `/api/`）；默认低并发 + 间隔，勿压站。
- 缓存目录 `data/sanguosha-cache/` 已被 `.gitignore` 忽略。

## 6. 牌表（人工录入，社区数据仅作参考）

- 官网 `sitemap.xml` 只有 `/hero/{gid}`、`/hero`、`/mode`、`/strategy` 等页面，**没有公开的牌表/卡牌列表接口**；`/strategy` 为攻略内容，不含结构化牌堆数据。
- 用户 2026-10-25 裁定：**牌表人工录入**。
- 为降低人工录入成本，已从社区开源包 `npm sanguosha-agent-plugin@1.17.0` 的 `STANDARD_DECK` / `JUNZHENG_DECK` 整理出 `cards.json`（标准 108 + 军争 52），并生成 `cards.md` 供人工逐张复核；**入库前必须人工核对花色/点数/数量**，不得直接视为官方数据。
- 坐骑距离按项目口径：**进攻马 +1 攻击距离 / 防御马 +1 防御距离**（见 `rules.md` §7）。
- 若后续找到官方结构化牌表，再以官方数据替换并复核。
