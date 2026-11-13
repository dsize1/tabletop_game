# v3-parity-baseline.md — v1 行为 oracle 对拍基线（M0）

> 状态：M0 进行中（2026-11-13 开工）。v1 已复活为 test-only oracle，本文记录对拍口径与白名单。
> 上游：ADR-0042 M0、`memory_palace/plan/next-v3-rebuild.md` §3-M0、
> `docs/games/sanguosha/engine-replacement/archive/parity-whitelist.json`（v1 已知缺陷白名单归档）。

## 0. 结论先说

- **v1 已复活**（git `f8a7db3^`），编译通过、**987 tests 全绿**，作为 V3 的**行为 oracle**（不注册网关、不上线）。
- **对拍口径**：同 seed + 同动作脚本回放，对比 **legal 动作集 / pending 序列 / 状态摘要（hp·手牌数·区域）/ 日志事件**；
  白名单外 0 差异。
- **v1 已知缺陷白名单 4 条**（按牌名枚举【杀】，影响火杀/雷杀判定），对拍时排除，避免「把缺陷当回归」。

## 1. v1 复活状态

| 项 | 值 |
|---|---|
| 复活来源 | git `f8a7db3^`（删除 v1 的前一个提交） |
| 位置 | `crates/engines/sanguosha`（已回 workspace members） |
| 规模 | 135 个 `.rs` 文件（`src/` 含 ai/resolve/resolve_*×20/skills/state/turn/view…） |
| 依赖 | `game-engine-core` + `serde`/`serde_json`（workspace 内，无新增） |
| EnginePlugin 对齐 | 与当前 `game-engine-core` trait 方法完全一致（game_id/rules/new_game/legal_actions/execute_action/public_state/player_view/summarize/validate_state/apply_scene） |
| 编译 | `cargo build -p sanguosha` 通过（2 warning：unreachable pattern / dead_code，无错误） |
| 测试 | `cargo test -p sanguosha` **987 passed / 0 failed** |
| 运行态 | 未注册进 `game-engine-mcp` / 网关（纯 test-only oracle） |

## 2. v1 已知缺陷白名单（对拍时排除）

> 来源：`engine-replacement/archive/parity-whitelist.json#v1_known_defects`（spike-report §12.2）。

| id | 位置 | 缺陷 | 影响 |
|---|---|---|---|
| `v1-chiyong-name-match` | `resolve.rs:6175` | 恃勇按物理牌名枚举【杀】（`sha\|huo_sha\|lei_sha`） | 语义判定，可能漏转化杀 |
| `v1-jiuchi-name-match` | `resolve.rs:6228` | 界酒池按物理牌名枚举【杀】 | 同上 |
| `v1-qiaomeng-name-match` | `resolve.rs` | 趫猛/利驭按物理牌名枚举【杀】 | 同上 |
| `v1-ai-is-sha-name-match` | `ai/card.rs:10` | AI 判断是否【杀】按牌名 | 只影响取舍，不参与规则 |

**注意**：v1 同类「按牌名枚举杀」仍有多处，白名单只登记已知/影响语义的 4 条；对拍时若发现新差异，先归入白名单再报警（防把缺陷当回归）。

## 3. v1 ↔ v2 已知差异（P0 / P1）

> 来源：`parity-whitelist.json#target_mode` + `v2_extra_defs`（M4 已对拍确认）。

### 3.1 P0（语义差异，需 V3 明确采用哪一侧口径）

| 牌 | v1 | v2 | 说明 |
|---|---|---|---|
| 杀 / 火杀 / 雷杀 | target_mode=None（按单目标） | Chosen（显式单目标） | 影响 legal 动作形状，语义一致 |
| 铁索连环 | None | Chosen（1–2 目标） | 语义差异：v1 未实现多目标 |
| 乐不思蜀 / 兵粮寸断 | None | Chosen（选目标） | 延时锦囊目标语义 |

### 3.2 P1（形状差异，不影响语义，白名单内）

| 牌 | v1 → v2 | 说明 |
|---|---|---|
| 无中生有 / 闪 / 桃 / 酒 / 闪电 / 无懈可击 | None/Single → SelfOnly | 只对自己/响应，主动使用被拒绝 |
| 木牛流马 | 无 → v2 额外生成 | 测试夹具用，不在 160 牌堆 |

## 4. 对拍机制设计（V3 批次开发的常驻工具）

### 4.1 对比维度（normalized，屏蔽两引擎 schema 差异）

| 维度 | v1 侧 | v2/v3 侧 | 归一化 |
|---|---|---|---|
| 合法动作集 | `legal_actions(state, pid)` | 同 | 排序后比对 id 集合（白名单外） |
| pending 序列 | `pending.kind` + actor | 同 | 比对「当前 pending kind + actor」序列 |
| 体力 | 各座位 hp | 同 | 座位序对齐 |
| 手牌数 | 各座位 hand.len() | 同 | 座位序对齐 |
| 区域 | 牌区归属（draw/discard/void/hand/equip/judge/mark） | 9 区 | 按牌 id 映射到归一化区域枚举 |
| 日志事件 | `game_events` | 同 | 按「事件类型 + 关键字段」归一化后比对 |

### 4.2 回放一致性前置（v1 已验证）

- v1 自带 `tests::replay::replay_reproduces_state_and_event_sequence` 与 `replay_is_stable_across_repeated_runs`（987 测试内，全绿）。
- 随机一律走 `RandomSource`/`rand_below`，同 seed 确定性 → 对拍可复现。

### 4.3 尚未落地（M0 后续 / V3 批次期）

| 项 | 说明 |
|---|---|
| replay 对拍脚本 | v1 与 v3 各跑同 seed 同动作脚本，输出归一化摘要 diff（当前只有口径，脚本待写） |
| v1 oracle 进 CI | 每次提交跑核心对拍（防漂移，§11.3） |
| 白名单有效性自检 | 检测白名单条目仍存在（防白名单腐烂，M4 已做一次） |

## 5. V3 使用方式

1. 每个迁移批次（B0→B20）实现后，同 seed 同动作脚本在 v1 与 v3 各跑一遍；
2. 对比 §4.1 的归一化维度，白名单外 0 差异才算收口；
3. 发现新差异 → 先判断「v1 缺陷 or v3 偏差」→ 归入白名单（若是 v1 缺陷）或修 v3（若是 v3 偏差）；
4. 白名单随 V3 推进逐步收紧（v1 已知缺陷在 V3 里应该已被修复，白名单条目最终归档）。
