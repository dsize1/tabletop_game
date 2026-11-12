# 交互原语规格（interaction）

## 1. 询问的判定（I1）

```
need_ask = 有可打出的选项（含转化/装备/技能替代）
         ∨ 手牌非空（即使没有对应的牌）
         ∨ 规则要求必须选择（强制选择步骤）
skip = !need_ask        // 只有这种情况允许不问
```

实现入口：`Game::must_ask(seat, has_option)`；三条响应路径（`card_response` / `wuxie` / `dying`）
与拆/顺取牌都必须经过它。

## 2. 出口（escape）语义

| 出口类型 | 语义 | 端侧 | 提交占位出口 |
|---|---|---|---|
| `escape(label)` | 可以不做（`pass`） | 展示 | 有效 |
| `escape_with(id,label)` | 取消 / 结束（`cancel` 等） | 展示 | 有效 |
| `escape_mandatory(label)` | 强制选择，出口只作类型占位 | **不展示** | **拒绝**（`该待办必须选择一项`） |

> 任何「出口 = 自动选第一个实质选项」的实现都是代玩家决策，禁止。

## 3. `pending.data` 契约

端侧（Web 浮层 / 个人区）通过 `pending.data` 读上下文，**帧数据不会投影到视图**：

| pending kind | 必填字段 |
|---|---|
| `card_response` | `step` / `target` / `card` |
| `wuxie` | `root` / `layers` / `count` |
| `dying` | `victim` / `hp` |
| `pindian` | `target` / `initiator` / `stage` |
| `harvest` | `pool` / `pickers` / `cursor` |
| `fire_attack` | `target` / `stage` |
| `card_pick`（拆/顺） | `card` / `target` |
| `collateral` | `step` |

## 4. 文案

- 同一 pending 内**文案不得对应不同后果**（`label_dups` 门禁，`check:sgs2-ai` strict 红灯）；
- 一张牌对多目标 → 每个目标一个选项且文案带目标名。

## 5. 并发与陈旧回答

- 响应窗口可能被嵌套流程（濒死 / 无懈 / 蛊惑）打断：回答已失效 → `Game::response_stale = true`
  → 调用方**重新询问**（不得按「未响应」结算）；
- 回答缺失（`g.answer == None`）同样走「重新询问」，禁止静默 `pass`。

## 6. 主公技接力（激将 / 护驾）

- **一个入口**：响应窗口选项（`jijiang` / `hujia`）或出牌阶段主动技（`skill:jijiang`）；
- **一个结果通道**：`Game::relay_result`（`Played{card,by}` / `Declined`），读回答前单次消费；
- 全部拒绝 → 回到主公本人的响应窗口（不是「未响应」）；
- 主动形态：代出的【杀】**视为主公使用**（次数/范围按主公、伤害来源=主公），牌不转手
  （`UseOpts::relay()` + `foreign_carrier`）；无人代出 = 零消耗、可再发。

## 7. 门禁

- `contracts/interaction.json`：禁止裸跳过；锚点指向 I1 用例；
- `tests/ask_completeness.rs`（7 条）+ `tests/b9.rs` 激将 5 条。
