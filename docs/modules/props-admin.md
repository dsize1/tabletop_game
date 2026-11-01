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
> 3. `/props-admin` 静态托管前先过 `propsSession`（会话 + `PROPS_ADMIN_WHITELIST`，默认回落 `DEV_WHITELIST`），非白名单 403；
> 4. 浏览器**不直连** server：`/internal/gameprops/*` 由 web 代理，代理前同样校验会话，server 侧再按「共享 token 或白名单 actor」二次判定。
> `PROPS_ADMIN_TOKEN` 降级为**本机/代理内部凭证**（web→server），不再由浏览器持有。

- `PROPS_ADMIN_TOKEN`：**web → server 的内部凭证**（与 server 同值）；浏览器不再需要输入（历史 localStorage token 输入保留兼容，但已不是准入主路径）。
- `PROPS_ADMIN_WHITELIST`：中台白名单（逗号分隔 openid）；未设置时回落 `DEV_WHITELIST`。
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

## API

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/internal/gameprops/summary` | 计数/状态分布 |
| GET | `/internal/gameprops/{table}?status=&q=&limit=&offset=` | 列表（白名单表） |
| GET | `/internal/gameprops/{table}/{code}` | 单行（模糊匹配） |
| POST | `/internal/gameprops/{table}` | upsert 一行（JSON） |
| POST | `/internal/gameprops/status` | `{table,entity_code,to_status,note?}` + 审计日志 |
| POST | `/internal/gameprops/seed` | 身份/配比/包初始化（幂等） |
| POST | `/internal/gameprops/import/heroes` | 导入 `heroes.json`（幂等） |

- 表白名单：`prop_packs` / `prop_assets` / `sgs_identities` / `sgs_identity_setup` / `sgs_card_defs` / `sgs_cards` / `sgs_generals` / `sgs_skills` / `sgs_general_skills` / `prop_status_log`。
- 生命周期：`draft / testing / review / live / offline`；每次流转写 `prop_status_log`。
- 导入的武将/技能状态为 `draft`，人工复核后上线；牌表按项目裁定人工录入（`cards.md` 仅参考）。

## 已知限制（P1）

- 图片 `image_asset_id` 只留字段，上传/裁剪未实现。
- 牌表未自动导入；需先在「牌定义」建定义，再到「定义↔物理牌」逐张加花色/点数。
- `sgs_cards` 的 `pack_id` 目前为数字输入；可在「道具包」先建标准/军争牌堆包。
- 准入 = Web 跨端登录会话 + 白名单（ADR-0031）；`/internal/gameprops/*` 不直连、由 web 代理；公网暴露由 `check:props-admin-boundary` 守护（白名单准入 + 不直连）。
- SPA 构建体积约 720KB（Naive UI 全量），后续可按需拆包。

## 测试

- `cargo test -p storage`：表创建、导入幂等、生命周期日志。
- `npm run build:props`：`vue-tsc` + Vite 构建。
- `npm run check:props-boundary`：校验公网 web 未挂载中台、server 仅监听 127.0.0.1。
- `cargo check -p server`：路由编译。
- 手工：`seed` → `import heroes` → 列表/状态流转 → web `/props-admin/`。
