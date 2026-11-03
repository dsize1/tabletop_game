# props-admin（道具中台，P1）

- **前端**：`apps/props-admin`（Vue 3 + Vite + TypeScript + Naive UI）
- **后端**：`crates/server` 的 `/internal/gameprops/*`（SQLite 单写者）
- **数据**：`crates/storage/src/gameprops.rs` + `CREATE_TABLES` 中的 `prop_*` / `sgs_*` 表
- **状态**：P1 可用（2026-10-26）；**仅本机**，不暴露公网；图片上传、卡牌官方导入尚未实现

## 启动

```bash
# 0) 一键开发链（默认包含中台，--no-props 可关闭）
npm run dev:up
# 中台：http://127.0.0.1:5174/props-admin/

# 或单独启动 server（默认 3000；这里以 3100 为例）
SQLITE_PATH=data/tabletop.db \
GAMEPROPS_DATA_DIR=docs/games/sanguosha/data \
SERVER_PORT=3100 \
PROPS_ADMIN_TOKEN=dev-token \
cargo run -p server --manifest-path crates/Cargo.toml

# 2) 开发模式（Vite，5174，代理 /internal → SERVER_URL）
cd apps
SERVER_URL=http://127.0.0.1:3100 npm run dev:props
# 打开 http://localhost:5174/props-admin/

# 3) 构建后本机预览（P1 不挂公网）
npm run build:props
cd apps
SERVER_URL=http://127.0.0.1:3100 npm --workspace @tabletop-game/props-admin run preview
# 访问 http://localhost:4174/props-admin/
```

> **安全边界（ADR-0031，2026-11-01 起）**：中台改为**与登录页同源、公网可达**，但准入 = **登录会话 + 白名单**：
> 1. 群内发 `#中台` → 机器人校验 `DEV_WHITELIST` 后回登录页链接（`scope=props`）；
> 2. 登录页完成「注册码 + 群内 `#验证码#NNNN`」确认 → web 写 `tg_props`（HttpOnly，8h）；
> 3. `/props-admin` 静态托管前先过 `propsSession`（会话 + 白名单）；非白名单 403；测试玩家（`TEST_PLAYER_WHITELIST`）只能进 `/props-admin/workbench`，其他路径 401（ADR-0035）；
> 4. 浏览器**不直连** server：`/internal/gameprops/*` 由 web 代理，代理前同样校验会话，server 侧再按「共享 token 或白名单 actor」二次判定。
> `PROPS_ADMIN_TOKEN` 降级为**本机/代理内部凭证**（web→server），不再由浏览器持有。

- `PROPS_ADMIN_TOKEN`：**web → server 的内部凭证**（与 server 同值）；浏览器不再需要输入（历史 localStorage token 输入保留兼容，但已不是准入主路径）。
- `PROPS_ADMIN_WHITELIST`：中台管理员白名单（逗号分隔 openid）；未设置时回落 `DEV_WHITELIST`。与 `DEV_WHITELIST` 取并集，均为 **Admin**。
- `TEST_PLAYER_WHITELIST`：测试玩家白名单（逗号分隔 openid），仅 **Workbench** 权限（只访问「工作台」菜单/API；手动改地址栏或越权 API 被拒绝）。Admin 优先于 Workbench（ADR-0035）。
- `GET /internal/gameprops/session`：返回 `{openid, scope, menus}`，SPA 用于裁剪菜单；`scope=admin|workbench`。
- 准入链路见 ADR-0031；开发模式（Vite 5174）仍需 `DEV_WHITELIST` 对应的登录会话。
- `GAMEPROPS_DATA_DIR`：`heroes.json` 所在目录，默认 `docs/games/sanguosha/data`。

## 页面

| 路径 | 内容 |
|---|---|
| `/` | 概览：各表计数/状态分布 |
| `/packs` | 道具包（武将包/牌堆包） |
| `/identities` | 身份（4 种） |
| `/identity-setup` | 人数 → 身份配比（5–8 + 双内） |
| `/card-defs` | 牌定义（分类/距离/装备槽/马类型/单-AOE） |
| `/cards` | 物理牌（花色/点数） |
| `/card-relation` | **牌定义 ↔ 物理牌关联页**（按定义筛选/新增） |
| `/generals` | 武将（势力/性别/体力/品质/描述/生平） |
| `/skills` | 技能（类型/描述） |
| `/general-skills` | 武将技能关联 |
| `/import` | 初始化基础数据、导入官网武将 |
| `/workbench` | **工作台**（P5 基座）：模板 / 预览 / AI 调试 / 决策日志 四 TAB |

## API

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/internal/gameprops/summary` | 计数/状态分布 |
| GET | `/internal/gameprops/{table}?status=&q=&limit=&offset=` | 列表（白名单表） |
| GET | `/internal/gameprops/{table}/{code}` | 单行（模糊匹配） |
| POST | `/internal/gameprops/{table}` | upsert 一行（JSON） |
| POST | `/internal/gameprops/status` | `{table,entity_code,to_status,note?}` + 审计日志 |
| POST | `/internal/gameprops/seed` | 身份/配比/包初始化（幂等） |
| POST | `/internal/gameprops/import/heroes` | 导入 `heroes.json`（幂等，`draft`：官网候选 696 位） |
| POST | `/internal/gameprops/import/engine` | **从引擎同步已制作数据**（幂等，`live`）：牌定义 43 / 物理牌 160 / 武将 232 / 技能 294 / 关联 443 |
| GET/POST | `/internal/gameprops/templates` | 工作台模板列表 / 新建（`game_id,name,source,snapshot,meta?`） |
| GET/PUT/DELETE | `/internal/gameprops/templates/{id}` | 模板详情 / 更新 / 删除 |
| POST | `/internal/gameprops/templates/{id}/duplicate` | 复制模板 |
| POST | `/internal/gameprops/templates/from-room` | 从当前对局快照抓取模板 |
| POST | `/internal/gameprops/templates/from-replay` | 从回放基线抓取模板 |
| GET | `/internal/gameprops/decision-logs?room_id=&since_seq=&limit=` | 决策日志只读（`player_id` 已哈希） |
| GET | `/internal/game/preview/meta` | 沙盒表单：149 去重武将 / 标准身份配比 / 默认包 |
| POST | `/internal/game/preview/sandbox` | **创建对局（沙盒开局）**：`{players,seed?,seats,save_template?,name?}` → 预览会话（全 bot、跳过选将） |
| POST | `/internal/game/preview` | 创建预览（`template_id` 或内联 `snapshot` + `scene`） |
| POST | `/internal/game/preview/scene` | 只校验/应用场景（不创建会话） |
| GET | `/internal/game/preview` | 列出内存中的预览会话 |
| GET/DELETE | `/internal/game/preview/{id}` | 预览视图（`raw=1`）/ 销毁 |
| POST | `/internal/game/preview/{id}/action` | 工作台代任意座位执行 |
| POST | `/internal/game/preview/{id}/drive` | AI 单步/连推（`max_steps`、`policy`） |
| POST | `/internal/game/preview/{id}/reset` | 回到模板/场景基线 |

- 表白名单：`prop_packs` / `prop_assets` / `sgs_identities` / `sgs_identity_setup` / `sgs_card_defs` / `sgs_cards` / `sgs_generals` / `sgs_skills` / `sgs_general_skills` / `prop_status_log`。
- 生命周期：`draft / testing / review / live / offline`；每次流转写 `prop_status_log`。
- 导入的武将/技能状态为 `draft`，人工复核后上线；牌表按项目裁定人工录入（`cards.md` 仅参考）。

## 引擎数据同步（2026-11-01）

中台表与引擎代码是**两层**：引擎（`crates/engines/sanguosha/src/{data,skills}.rs`）是「已实现」的唯一真相，中台表只由导入写。

- 「数据导入」页 → **同步引擎数据**（`POST /internal/gameprops/import/engine`）：写入牌定义/物理牌/武将/技能/关联，状态 `live`。
- 幂等口径：牌定义/武将/技能按 `code` 覆盖；物理牌用**保留 id 段**（`1_000_000` 起）先清后写——因为真实牌堆允许「同牌名 + 同花色点数」重复（标准包 108 张里 11 组重复），**`sgs_cards` 不再建 `(pack_id, def_code, suit, rank)` 唯一索引**（历史索引由迁移 `DROP INDEX` 清理）。
- 「官网武将」导入（`heroes.json`，`draft`）保留：用于未实现武将的候选元数据（696 位），与引擎同步互补。
- 包 code 与引擎 `GENERAL_PACK_KEYS` 对齐（`standard` / `wind_fire_forest_mountain` / `yijiang` / `jiexian`）；`seed` 已改用 `jiexian`（旧 `boundary_break` 会残留空行，需手工删除）。

## 工作台（P5 基座，ADR-0033）

- 页面 `/workbench`：模板（从对局/回放抓取、**创建对局抽屉**、JSON 导入、复制、删除、导出、`meta.scene` 编辑）、预览（iframe + 创建/重置/销毁/视角）、AI 调试（raw 状态、合法动作、单步/连推轨迹）、决策日志（只读查询 + JSONL 导出）。
- **创建对局（沙盒开局，ADR-0034）**：5–8 人、逐座位指定武将/身份（不选随机）；身份按标准配比、已选身份/武将其他座位置灰；全 bot；跳过选将、主公先手、随机牌堆/手牌；可选 seed 复现、可选保存模板；创建后停在主公出牌阶段/开局 pending，不自动连推。
- 预览是**独立内存态**：不写 `game_state` / `game_events` / `game_decision_log`，重启即丢；默认空闲 2h、最多 8 个会话（`SGS_PREVIEW_TTL_SECS` / `SGS_PREVIEW_MAX_SESSIONS`）。
- 预览页 `/tabletop_game/:gameId/preview` 走中台会话（`tg_props` + 白名单），**不要求**玩家 token；预览 API 由 web 代理，浏览器不直连 server。
- **本地开发注意**：Vite（5174）与 web（18080）跨站，`SameSite=Lax` 的 `tg_props` 不会随 iframe 发送。要在本地看 iframe 预览，请用同源构建页：`npm run build:props` 后打开 `http://127.0.0.1:18080/props-admin/workbench`。
- 场景注入 `SceneSpec` 字段见 `crates/engines/sanguosha/src/scene.rs` 的 `scene_schema()`；卡牌 token 只支持「牌实例 id / 定义码」，且只能从摸牌堆/弃牌堆/移出区取。

## 已知限制（P1）

- 图片 `image_asset_id` 只留字段，上传/裁剪未实现。
- 牌表未自动导入；需先在「牌定义」建定义，再到「定义↔物理牌」逐张加花色/点数。
- `sgs_cards` 的 `pack_id` 目前为数字输入；可在「道具包」先建标准/军争牌堆包。
- 准入 = Web 跨端登录会话 + 白名单（ADR-0031）；`/internal/gameprops/*` 不直连、由 web 代理；公网暴露由 `check:props-admin-boundary` 守护（白名单准入 + 不直连）。
- SPA 构建体积约 720KB（Naive UI 全量），后续可按需拆包。

## 测试

- `cargo test -p storage`：表创建、导入幂等、生命周期日志。
- `npm run build:props`：`vue-tsc` + Vite 构建。
- `npm run check:props-boundary`：校验公网 web 未挂载中台、server 仅监听 127.0.0.1、预览代理同属白名单门禁。
- `npm run check:sgs-preview`：临时 server + MCP 验证模板/场景/预览/AI/重置/销毁/沙盒创建与隔离（需 Redis + 已构建 server/MCP）。
- `npm run check:sgs-preview:ui`：追加临时 web + Playwright（系统 Edge）冒烟：未登录 403、预览页工具栏/视角/AI/重置、工作台四 TAB、iframe 内预览；并验证测试玩家仅工作台菜单（首页/其他菜单 401、越权 API 403）。缺浏览器/`playwright-core` 时自动跳过。
- `cargo check -p server`：路由编译。
- 手工：`seed` → `import heroes` → 列表/状态流转 → web `/props-admin/`。
