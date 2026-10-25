# ADR-0017：桌游道具中台 —— 数据模型、写入边界与单页技术栈

- **状态**：提案·待评审（2026-10-25）
- **相关**：`docs/games/sanguosha/plan.md`（一期需求）、`docs/games/sanguosha/data-model.md`（已评审 v0.2）、`docs/games/sanguosha/development-plan.md` P1；ADR-0001（SQLite 真相源）、ADR-0004（状态序列化）、ADR-0010（版图渲染与素材流水线）、ADR-0016（规则内核刚性）、ADR-0018（设计工作台）
- **结论一句话**：新建 `apps/props-admin`（**Vue 3 + Vite + TypeScript + Naive UI**）+ `crates/server` 的 `/internal/gameprops/*` 受控 REST；表结构采用「通用包/素材表 + 三国杀专用强类型表」；所有道具/武将带**生命周期状态**；提供**牌定义 ↔ 物理牌关联页**；**武将数据已从官网全量抓取**（696 位 / 1383 技能）可直接导入，**牌表无官方公开结构化接口**（人工/社区整理）；图片字段先留空，规则校验不在中台。

## 背景

1. 立项文档要求第一阶段先做道具中台：身份牌、手牌（含花色/点数/分类/距离/单-AOE）、武将（含体力/技能列表/技能类型）、图片（先留字段）与描述。
2. 「每类道具都要有图片 + 描述」；身份牌需要初始化描述；血条牌不入中台；手牌花色/点数初始为空。
3. 中台还需要维护后续工作台用的模板（局面/资源）与武将流程设计绑定（P5/P6）。
4. 现有 Web（`apps/web`）是 Express SSR + 纯 TS 视图 + esbuild，ADR-0010 明确**不引入 React/Vite**；那套适合只读展示，不适合表单/表格/上传为主的管理后台。
5. 现有持久化约定：SQLite 单写者收敛到 Rust `server`；Web/SPA 不直连 SQLite；素材有 `apps/scripts/board-assets.mjs` 流水线可复用。

## 决策

### 1. 前后端边界

- **后端**：`crates/server` 新增 `/internal/gameprops/*`（只增不影响现有 `/internal/*`）：
  - `GET /packs`、`GET /cards`、`GET /generals`、`GET /skills`、`GET /identities`（列表/筛选/分页）；
  - `POST/PATCH/DELETE` 对应资源（管理员白名单）；
  - `POST /import`（幂等 seed / `heroes.json` 武将导入 / 后续牌表导入）；
  - `GET /export`（JSON，供引擎 fixture 与文档生成）；
  - 生命周期流转 `POST /{resource}/{code}/status`（新建→测试→审核→上线→下线软删）。
- **鉴权**：沿用「管理员白名单」模式（类似 `DEV_WHITELIST`），新增 `PROPS_ADMIN_WHITELIST`（openid）；失败 403；不引入新的 token 体系。
- **写入**：所有写操作在 server 内做参数校验（枚举、唯一键、血量 `initial_hp <= max_hp`、距离范围），并写审计（`updated_at` + 操作者）。
- **SPA**：`apps/props-admin`，静态构建产物由现有 web/Express 或 server 静态托管（二选一，P1 定）；开发期 Vite dev server 代理到 server。页面含：身份配置、牌定义、物理牌、**定义↔物理牌关联页**（按定义筛选/批量生成/绑定花色点数）、武将、技能、包管理、生命周期流转。

### 2. 数据模型（详见 `docs/games/sanguosha/data-model.md`）

- 通用：`prop_packs`（包）、`prop_assets`（素材登记，可空引用）。
- 身份：`sgs_identities` + `sgs_identity_setup`（人数 → 身份配比，支持 8 人双内变体）。
- 牌：`sgs_card_defs`（定义/分类/距离/目标类型/**装备槽位 equip_slot**/描述/图）× `sgs_cards`（物理牌：花色 + 点数 + 所属包）。
- 生命周期：所有身份/牌/武将/技能行统一 `status`（新建/测试/审核/上线/下线软删）；`prop_status_log` 记录迁移/审核；只读接口只取 `live`（见 `data-model.md` §3.5）。
- 武将：`sgs_generals` + `sgs_skills` + `sgs_general_skills`。
- 模板/决策/工作台：`game_state_templates`、`game_decision_log`、`sgs_flow_designs`（P5–P6 启用）。
- 运行时状态**不建实体表**：沿用 `game_state` + `game_events`（ADR-0004/0008）。

设计取舍：

- 牌「定义 × 物理牌」两分：避免 108 张牌重复描述/图片；花色与点数是物理属性（同一「杀」有多种花色点数）。
- 强类型列 + `CHECK` 枚举，而非统一 `props` 表 + JSON：后台表单可直接校验、SQL 可查询、迁移可控；未来新桌游加自己的 `xxx_*` 表，通用表不变。
- 技能类型只存元数据：**不存触发逻辑**（ADR-0016）。

### 3. 前端技术栈

| 方案 | 结论 |
|---|---|
| 继续纯 TS + esbuild（同 `apps/web`） | 否：大量表单/表格/上传/校验，自研成本高于框架 |
| React + Vite + Ant Design | 可行，但与现有 Web 栈分裂、包体更大；团队无既有 React 工程 |
| SvelteKit | 轻，但后台组件生态与图编辑（后续）较弱 |
| **Vue 3 + Vite + TypeScript + Naive UI** | **采纳**：后台组件齐全（表格/表单/上传/树）、体积可控、中文生态成熟；与 n8n 前端同为 Vue 系，后续工作台协作成本低 |

- 路由：Vue Router；状态：Pinia（仅少量本地状态）；HTTP：原生 `fetch` 封装（不引 axios）。
- 图编辑（P5）：优先复用 n8n；撤退方案用 Vue Flow（Vue 生态），避免为图编辑再引一套 React。
- 不引入 SSR：内部后台，纯 SPA；首屏体积与 SEO 无要求。

### 4. 素材（图片）策略

- P1 只建 `prop_assets` 与 `image_asset_id` 可空字段，不做上传；展示走占位图。
- 后续接 `apps/scripts/board-assets.mjs` 同思路：本地文件 → 紧裁/压缩 → webp → 内容指纹命名 → 登记 `prop_assets`；中台只选图，不做在线绘制。
- 素材来源：**后续自行制作，不使用官方图片**（用户 2026-10-25 确认）。

### 5. 初始数据（seed）

- 身份 4 种 + 人数配比（5–8，含 8 人双内变体）与身份描述。
- 武将：用 `docs/games/sanguosha/data/heroes.json`（官网 696 位）幂等导入；`gender` 缺失留 `unknown`，技能类型 `type_guess` 人工复核。
- 牌：标准包定义 + 军争定义（酒/火杀/雷杀等，后续玩法启用）；物理牌**花色/点数先留空**；牌表**无官方公开结构化接口**，P1 人工录入/社区整理表校验。
- 武将：标准包 25 将骨架（名称/势力/性别/体力/包），技能与描述逐步补齐。
- seed 脚本幂等（`INSERT OR IGNORE` / upsert by code）。

## 理由（取舍）

- 中台是**内部工具**，不是面向玩家的产品：优先「可维护、可校验、可迁移」，不追求炫技。
- 规则内核刚性（ADR-0016）与「中台只配元数据」是一条硬边界：避免出现「在后台改一个字段就改变线上规则」的不可审计路径。
- 数据模型选择强类型表而非 JSON：三国杀字段稳定且需要约束；JSON 方案会在表单校验与查询上付出更高成本。

## 影响

- 新增 workspace `apps/props-admin`、server 路由模块、`crates/storage` 迁移与 seed/导入脚本、`apps/scripts/crawl-sanguosha-heroes.mjs`（可复现抓取）。
- 官网数据版权归官方；`heroes.json` 仅内部开发/初始化，上线前确认授权。
- 生命周期状态影响只读接口：引擎 fixture、牌表导出、Web 资源接口都只取 `live`；软删不改历史回放。
- 「定义↔物理牌关联页」是 P1 必交付项（用户 2026-10-25 明确）。
- `docs/games/sanguosha/data-model.md` 成为**审核基线**：确认后建表；后续字段变更走迁移 + ADR 增补。
- 部署：多一个静态站点/路由；不影响现有 `apps/web` 与 QQ 链路。
- 与 ADR-0010 的关系：不冲突——`apps/web` 仍不引 React/Vite；管理后台是独立应用。

## 依据与验证状态

- 中台字段需求来源：`docs/games/sanguosha/plan.md`（用户立项文档）；数据模型已于 2026-10-25 评审（结论见 `data-model.md` §7）。
- 「SQLite 单写者 / 不直连」来源：`docs/conventions/persistence.md`，**已验证**（现有 dev/props 类似路径）。
- Vue 3 + Vite + Naive UI 的体积/维护性判断为工程经验，**待验证**：P1 首个页面完成后记录构建体积与冷启动时间；若超出预期再评估 Svelte/Preact（不改变 API 与数据模型）。
- 本 ADR 不新增外部数值约束（无频控/配额）。
