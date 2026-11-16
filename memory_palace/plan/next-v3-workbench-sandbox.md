# V3 工作台沙盒 / 场景注入 / 预览 drive 适配（步骤 D 前置）

> 状态：**已实施（S1–S4 ✅ 2026-11-16）** · 创建 2026-11-16
> 触发：`next-v3-verification.md` 步骤 D 实测阻塞——workbench 的 sandbox/scene/drive 仍是 v2 基座，
> `audit:sgs-generals` 无法产出 V3 机器证据（`游戏 sanguosha 不支持场景注入`）。
> 本任务只做「预览基座 V3 化」，不改任何技能行为；技能台账见 `next-v3-verification.md`。

## 0. 事实基线（2026-11-16 核查）

| 事实 | 证据 |
|---|---|
| `POST /internal/game/preview/sandbox` 硬编码 v2 | `crates/server/src/preview.rs`（`sanguosha2::sandbox::new_sandbox_game`，`SandboxRequest.seats: sanguosha2::sandbox::SandboxSeat`） |
| `GET /internal/game/preview/{id}` 的 `seat_summary` 按 v2 `Game` 解析 | `preview.rs::PreviewSession::seat_summary`（V3 会话返回 `seats: []`） |
| `POST .../drive` 用 v2 AI（`current_actor` / `decide_traced`） | `preview.rs` 驱动段；V3 会话 `stop=no_actor` |
| V3 `SanguoshaEngine` 未实现 `apply_scene` | `game-engine-core::EnginePlugin` 默认实现返回「不支持场景注入」 |
| V3 已有可复用开局/组牌堆/回合驱动 | `sanguosha3::engine::lobby::{finish_setup, build_deck}`、`kernel::turn::{new_game, begin_turn, start, skip_phase}` |
| V3 私有视图已含 pending 选项 | `sanguosha3::kernel::view::player_view` → `my_pending.{options,data,escape}`（宪法 I3：公共视图不暴露） |

## 1. 目标

1. `/internal/game/preview/sandbox` 可按 `game_id` 创建 **V3** 已开局沙盒（武将/身份/seed）；
2. V3 引擎实现 `apply_scene`（手牌/装备/判定/翻牌/横置/回合/阶段/牌堆顶/计数器）；
3. 预览 `seat_summary` / `drive` 去 v2 耦合（引擎能力分派，不再 `sanguosha2::ai`）；
4. `audit:sgs-generals` 读 `player.my_pending.options`，跑通 B0 dry-run 与全量 21 批；
5. 证据头写 `engine=v3`，并加门禁防 v2 证据复用。

## 2. 设计（分层，尽量不新增引擎语义）

### 2.1 core：能力接口（`crates/game-engine-core/src/lib.rs`）

`EnginePlugin` 新增（默认保守实现，v1/v2 不受影响）：

```rust
/// 是否支持工作台沙盒（缺省 false）。
fn supports_sandbox(&self) -> bool { false }
/// 创建工作台沙盒（已开局快照）；缺省返回 IllegalAction。
fn create_sandbox(&self, spec: &serde_json::Value, rng: &mut dyn RandomSource)
    -> Result<GameState, EngineError> { Err(EngineError::IllegalAction("不支持沙盒".into())) }
/// 当前待操作玩家（预览 drive 用）；缺省 None。
fn current_actor(&self, state: &GameState) -> Option<String> { None }
/// 座位摘要（预览/工作台展示）；缺省从 `state.players` 推导最小字段。
fn seat_summary(&self, state: &GameState) -> serde_json::Value { /* players + index */ }
```

> 理由：`preview.rs` 不应认识具体引擎；v2 保留现有 `apply_scene` 与 sandbox 行为，逐项改为实现该接口
> （v2 可先仅实现 `supports_sandbox/create_sandbox/current_actor/seat_summary` 包装现有代码）。

### 2.2 V3：沙盒模块（新增 `crates/engines/sanguosha3/src/sandbox.rs`）

- `SandboxSpec { players, seed, seats: [{ index, general?, identity?, name? }] }`
- `create_sandbox`：
  1. `turn::new_game(players)` + 座位 `name = sN`（玩家 id 稳定、可复现）；
  2. 身份：座位 0 = `Lord`，其余按入参或默认（`Rebel`）；
  3. 武将：`generals::def(code)` → `apply_to_seat` + **挂载已收口技能**（与 `lobby::finish_setup` 同一段逻辑，
     抽出 `mount_general(g, seat, code)` 复用，避免两套漂移）；
  4. 组牌堆：`lobby::build_deck(g, cfg.packs, rng)`（默认包组合与真局一致）；
  5. 初始手牌 4 张（与真局口径一致）→ `g.started = true`；**不**进入首个回合（由 scene 决定起点）。
- `apply_scene(g, scene)`（`EnginePlugin::apply_scene` 转调）：
  - `round/turn/turn_phase/phase`、`deck_top`（数组首元素=牌堆顶）、`counters`；
  - `seats[]`：`hand` / `hand_append` / `equips` / `judge` / `chained` / `flipped` / `hp` / `max_hp` / `identity` / `general`；
  - 牌令牌按 `card_defs[id].def == token` 从**牌堆/弃牌堆**取（与 v2 `take_card` 同口径，禁止从他人手牌取）；
  - 所有区域变更走 `kernel::flow`（`to_hand`/`to_discard`/`to_deck_top`/`gain`）或新增 `sandbox_move` 单出口，
    保证 `CardIntoGame/CardLost` 事件守恒（审计用牌数守恒）。
  - **阶段落点**：清空栈/待办 → `turn::begin_turn(g, turn)` → `g.skipped = [Prepare, Judge, Draw]`（按 `turn_phase` 需要）
    → push `Turn` 流程帧 → `resolve::advance`；断言最终 `g.phase == 目标阶段` 且 pending 合法。
- 确定性：`SeededRng` 只用于组牌堆；`g.rng = seed | 1`，同 seed 同场景 → 同 digest（审计不变量）。
- 不写真相源：与现有 preview 一致（仅内存会话）。

### 2.3 server：预览去 v2 耦合（`crates/server/src/preview.rs`）

- sandbox 端点：`state.games.registry().get(game_id)` → `supports_sandbox()` → `create_sandbox(spec, rng)`；
  请求体 `seats` 改为通用 `serde_json::Value`（v2 分支内部反序列化为 `sanguosha2::sandbox::SandboxSeat`）。
- `seat_summary` → `engine.seat_summary(&state)`；
- `drive` → `engine.current_actor(&state)` + `mgr.legal_actions` + `RandomPolicy.choose`；
  保留「同一 (actor, action) 连续 8 次破环」与 `max_steps`、`viewer` 语义。
- `/internal/game/preview/meta`：按引擎返回（v3 用 `generals_generated` 147 将 + 标准身份配比），v2 保持原样。

### 2.4 审计脚本（`apps/scripts/sgs-general-audit.mjs`）

- 读 pending 选项改走 **私有视图**：`body.player.my_pending.options`（V3）/ `body.raw.data.pending.options`（v2 兼容）；
- 动作来源继续用 `body.actions`（`legal_actions`，已按 `viewer/actor` 下发）；
- `--no-scene` 用 `create_sandbox` 快照直接建 preview（不注入 scene）；
- 证据头加 `engine`（v2/v3）与 `skills` 数（299），并记录 `server_build`（避免旧二进制）。

### 2.5 门禁（防「又跑成 v2」）

- `check:sgs-preview`：新增 V3 用例（沙盒可建 / scene 可注入 / 同 seed digest 一致 / pending 可续 / 牌数守恒）；
- `sgs-general-audit --check`（抽样门禁）：证据必须是 `engine=v3`（从 preview 视图的 `game_id` 判定）；
- `general-audit-evidence.md` 头必须含 `引擎：v3`，否则 `check:sgs-generals:matrix`（或新校验）告警。

## 3. 待办（分 4 步）

- [x] **S1** core 能力接口 + v2 包装（`supports_sandbox/create_sandbox/current_actor/seat_summary`）
- [x] **S2** `sanguosha3::sandbox`：`mount_general` 抽取、`create_sandbox`、`apply_scene`、确定性取牌（按实例 id 排序）
- [x] **S3** `preview.rs` 去 v2 耦合（sandbox 分派 / seat_summary / current_actor / drive 通用策略）；`sandbox_meta` 仍 v2（工作台表单待后续）
- [x] **S4** 审计脚本读 `player.my_pending.options` + 证据头 `引擎：v3`；B0 dry-run → 全量（触达 86 / 未触达 61 / 失败 0 / 阻塞 0）

## 4. 验证事项

- [ ] `cargo build -p server` 绿（当前已修 `Suit::None`）
- [x] `npm run check:sgs-generals`（抽样 8 将 repeat=2）失败 0 / 阻塞 0（确定性修复：场景取牌按实例 id 排序）
- [ ] `npm run check:sgs-preview` 绿（V3 用例尚未补：工作台 UI 仍走 v2 表单）
- [x] `npm run audit:sgs-generals -- B0 --seed 20261105`：失败 0 / 阻塞 0，且 `game_id=sanguosha`
- [x] `npm run audit:sgs-generals:all` → `general-audit-evidence.md` 头 `引擎：v3`，失败 0 / 阻塞 0（触达 86 / 未触达 61）
- [x] `npm run check:sgs3:full` 无回归（技能注册/契约不受影响）
- [ ] `npm run sgs:check:fast`（下一步验证）
- [ ] 回放隔离：预览会话不写 `game_state/game_events/game_decision_log`（沿用现有断言）

## 5. 风险 / 注意

- **场景注入不得绕过单出口**：不得直接写 `cards`/`seats` 造成事件缺失（审计守恒会红）；新增 `sandbox_move` 需自带事件。
- **与真局的阶段口径一致**：`turn::skip_phase` 在 `begin_turn` 会清空 `skipped`，场景落点必须在 `begin_turn` **之后**设置。
- **不允许为审计放宽引擎校验**：`validate_state` 仍须拒绝非法快照。
- **不得把 v2 证据当 V3**：证据头 engine 与 `check:sgs-general` 抽样门禁双重校验。
- 本任务不动技能实现；若沙盒暴露技能缺陷 → 记入 `general-audit.md`，按 `next-general-audit.md` 走。

## 6. 实施记录（2026-11-16）

- 新增 `crates/engines/sanguosha3/src/sandbox.rs`（建局/场景注入/座位摘要/确定性取牌）。
- `core::EnginePlugin` 新增 `supports_sandbox / create_sandbox / current_actor / seat_summary`（默认保守）。
- `preview.rs`：sandbox 由引擎 `create_sandbox` 建局；`seat_summary/current_actor/drive` 去 v2 硬编码。
- 审计：`collect()` 读 `player.my_pending`，`countCards` 支持 `kernel.cards`；证据头写 `引擎：v3`；抽样不再覆写全量证据。
- 顺带修复：`skills::start_active` 不再注入 `step:"choose"`（13 个主动技「未知阶段 choose」全部消失）。
- 结果：147 将 · 触达 86 / 未触达 61 / 失败 0 / 阻塞 0；`check:sgs-generals` 抽样失败 0。
