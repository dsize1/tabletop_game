# ADR-0037：对局页客户端特效层 —— 历史增量消费、输入锁定与 ComfyUI 素材管线

- 状态：已采纳（2026-11-04）
- 关联：ADR-0010（版图素材）、ADR-0036（卡面入中台）、`docs/modules/web.md`、`docs/games/sanguosha/ui-spec.md`

## 背景

1. 对局页每 10s 无条件轮询 `/state`，并在每次 `applyState` 重建 `#sgs-app`；但状态机在等待本人选将/响应/出牌时不会推进，轮询没有收益且会闪屏。
2. AI 连续动作在一次动作请求内完成，客户端一次轮询只能看到最终状态 + 一批 `public_board.history` 文本；玩家要翻历史才知道发生了什么。
3. 约束：**不改 Rust server / 引擎字段**，只能改 web（客户端、视图纯函数、静态素材、离线生成脚本、文档）。

## 决策

1. **轮询降频**：新增 `awaitingLocal()`（本人 pending / 选将未选 / 本人回合有合法动作）→ 60s 保活间隔；离开待办态恢复 10s。提交动作、visibility/focus/pageshow/online 立即刷新。
2. **重绘抑制**：`appSig`（不含 history）与 `sideSig`（history 末 seq + names）拆分；仅 history 变化时只刷侧栏，完全无变化只更新时间戳。
3. **特效数据源**：以 `history` 文本解析为主、`seats[].hp` 差分兜底；不追加后端字段。文案契约表驱动，解析失败静默降级。
4. **特效层**：客户端运行时注入 `#sgs-fx-layer`（fixed、pointer-events:none、z-index 95），不改变现有 SSR/DOM 结构；座位定位用 `[data-seat]` / `.sgs-personal` 的实时矩形。
5. **消费与锁定**：首帧只建 `lastHistorySeq` 基线；后续增量按 seq 消费。队列 >12 条只播最近 12 条，同目标同类型合并；消费期间**禁止操作**（`body.sgs-fx-locked` + `fxLocked` 前置判断 + 暂停轮询），允许滚动右侧历史，结束后立即刷新恢复。不提供跳过按钮。
6. **ComfyUI 素材**：Animagine XL 3.1 生成黑底 VFX → Pillow 亮度键控/裁剪 → **168px 五帧透明动画 WebP**（`normal` 混合，`/fx-assets` immutable）；运动来自帧序列 + 淡入淡出，不依赖 AnimateDiff。`gen:fx -- --assemble` 支持用户改图后重打包。
7. **可访问性**：`prefers-reduced-motion: reduce` → 描边/文字提示，不闪屏；雷电全屏闪 ≤2 次、透明度 ≤0.35；特效只改 `transform/opacity/filter`。

## 理由与备选

- **备选（弃）**：改后端加结构化事件/动画指令。违背本轮约束，且历史文本已足够覆盖当前 9 类效果。
- **备选（弃）**：跳过按钮。用户明确要求「按过渡效果消费」，改为队列限时（≤3s）+ 最近 12 条。
- **备选（弃）**：AnimateDiff/视频素材。本机无 AnimateDiff/RIFE 模型，SDXL 静态图 + CSS 即可，素材体积更小。
- **备选（弃）**：透明 PNG。黑底 + `mix-blend-mode:screen` 更稳；亮度键控只用于裁边/压体积。

## 影响

- 新增 `apps/web/src/view/sgs/{poll,fx,fx-manifest}.ts`、`apps/web/src/client/sgs-fx.ts`、`apps/web/public/fx/*`、`apps/scripts/{gen-fx-assets,check-fx-assets}.mjs`、`apps/scripts/fx_pack.py`。
- `client/sgs.ts` 接入轮询策略/签名/特效队列/锁定；`index.ts` 增加 `/fx-assets` 静态路由。
- 不改 Rust server / 引擎；`check:props-boundary` 等既有门禁不受影响。
- 预算：单素材 ≤120KB、总 ≤1.2MB（当前 9 个效果 871KB，168px × 5 帧，透明 alpha）。

## 依据与验证状态

| 约束 | 值 | 来源 | 验证状态 |
|---|---|---|---|
| 历史条数上限 | 800 | `HISTORY_LIMIT`（引擎） | 已验证（代码） |
| 特效消费上限 | 12 条 / ≤3s | 用户裁定 | 实现约束 |
| 待办轮询 | 60s 保活 | 用户裁定 | 实现约束 |
| 素材预算 | 单 ≤120KB，总 ≤1.2MB；168px × 5 帧透明 WebP | 内部预算 | 已验证（`check:fx-assets`） |
| 特效运行时锁定 | 操作禁用、历史可滚 | 用户裁定 | 已验证（DOM 手动） |
