# 状态模型规格（state）

## 1. 体力与存活

| 字段 | 语义 | 写入约束 |
|---|---|---|
| `Seat.hp` | 当前体力（可为 0，濒死时保持 0） | 只能经 `flow::damage*` / `flow::heal` / `flow::lose_hp` / `flow::clamp_hp` / `flow::revive_*` |
| `Seat.max_hp` | 体力上限（开局 = 武将表值，主公 +1） | 开局 `lobby::finish_setup`；技能改上限须同时处理体力钳制 |
| `Seat.alive` | **存活**（唯一事实） | 只由 `flow::die` 置 `false`；初始化 `true` |

**不变量**

- `alive == true && hp <= 0` **只允许**出现在濒死结算（栈内有 `FlowKind::Dying` 帧）；
- `alive == false` → 不再被询问、不再是合法目标、回合跳过；
- 死亡确认后身份公开；`alive == true` 时身份除主公外不得公开。

**死亡路径（唯一）**

```
flow::damage*/lose_hp → hp <= 0 → flow::start_dying（压 Dying 帧）
  → step_dying 逐个询问救援（I1：有手牌必问）
      ├─ 救起（hp > 0）→ leaving_dying / settle_after_dying → Done（alive 保持 true）
      └─ 无人救援（cursor ≥ order.len()）→ flow::die（alive = false）→ 死亡清理
```

## 2. 身份

| 身份 | 公开时机 |
|---|---|
| `lord` | 始终公开 |
| `loyalist` / `rebel` / `traitor` | 仅 `alive == false` 后公开 |

视图投影：`identity = (lord || !alive) ? Some(identity) : None`（`view.rs::public_seat`）。

## 3. 开局

- 选将前：`hp = 0`（未初始化）。视图 `hp / max_hp / alive` **一律 null**（不能展示 0/0，也不能据此判身份）；
- 选将收尾 `lobby::finish_setup`：`max_hp = 武将表 max_hp (+1 主公)`，`hp = max_hp`；
- 沙盒开局（`sandbox::new_sandbox_game`）同口径（用例 `state_semantics::sandbox_game_starts_at_full_hp`）。

## 4. 区域

`Hand / Equip / Judge / Mark / Draw / Discard / Void / Processing / Frame` 九类，
投影规则见 `docs/conventions/persistence.md` 与 `tests/view_contract.rs`；本规格只约束：
**未经引擎移动不得跨区域**（守恒 `Game::check_conservation`）。

## 5. 门禁

- `Game::check_no_zombies()`（每次动作后校验）；
- `contracts/state.json`：`view.rs` / `identity.rs` 禁止 `hp` 比较；
- 用例：`tests/state_semantics.rs`。
