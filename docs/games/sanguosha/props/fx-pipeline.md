# 对局特效素材流水线（ComfyUI，ADR-0037）

- **用途**：三国杀对局页把 `public_board.history` 的增量消费成过渡特效（挥刀/闪避/桃/酒/雷/火/铁索）。
- **约束**：**不改后端**；特效只做「已发生事实」的视觉呈现，不参与规则/合法动作判定。
- **运行时**：`apps/web/src/client/sgs-fx.ts` 注入 fixed 图层；素材 `apps/web/public/fx/`；清单 `apps/web/src/view/sgs/fx-manifest.ts`。
- **素材格式**：**透明背景的动画 WebP**（168×168，5 帧 @80ms，alpha 通道），正常混合（`normal`），不依赖 `mix-blend-mode:screen`。

## 效果与素材

| id | 触发（历史文本/差分） | 运行时 |
|---|---|---|
| `slash` | 普通伤害 `X 受到 N 点普通伤害` / HP 下降兜底 | 目标座位挥刀、命中闪光 |
| `dodge` | `打出【闪…】` | 打出者侧闪 + 刀光落空 |
| `peach` | `使用【桃…】` / 救援 | 桃 + 绿色回血光环 |
| `wine` | `使用【酒…】` | 酒葫芦 + 红金怒气 |
| `thunder_bolt` | `受到 N 点雷电伤害`（雷杀/雷击/闪电） | 座位雷击；`雷击/【闪电】` 前置时用加强变体 |
| `thunder_flash` | 同上（加强变体全屏闪） | 全屏雷光（≤2 次、≤0.35 透明） |
| `fire` | `受到 N 点火焰伤害`（火杀/火攻） | 火焰灼烧 + 橙色描边 |
| `chain` | `铁索连环：属性伤害传导/对…结算` | 相关座位逐个链缚 |
| `heal` | `回复 N 点体力`（非桃） | 绿色回复光环 |

## 环境（本机实测）

- `C:\llm\ComfyUI` + `C:\llm\comfy-venv\Scripts\python.exe`
- checkpoint：`animagine-xl-3.1.safetensors`（SDXL base / SD1.5-inpaint 备用）
- `custom_nodes`：仅 `ComfyUI_IPAdapter_plus`；`background_removal` / `frame_interpolation` 为空 → **不依赖 AnimateDiff/RIFE**，用静态图 + CSS 动画。
- 若 GPU 被决策服务占用：先 `npm run dev:down`，生成后再 `npm run dev:up`。

## 生成 / 改图重打包

```bash
# 0) 启动 ComfyUI（默认 8188）
C:/llm/comfy-venv/Scripts/python.exe main.py --listen 127.0.0.1 --port 8188   # cwd: C:/llm/ComfyUI

# 1) 首轮生成（9 个效果；源图落 assets-src/fx/src/*.png，本地 gitignore）
npm run gen:fx
npm run gen:fx -- --only slash,fire --force

# 2) 你修改源图后（assets-src/fx/src/*.png）——不重跑 ComfyUI，只重打包：
npm run gen:fx -- --assemble

# 3) 门禁 + 构建
npm run check:fx-assets
npm --prefix apps/web run build:client
```

- `--assemble`：`apps/scripts/fx_pack.py`（Pillow）做亮度键控（alpha = max RGB）→ 裁空白 → 168px 五帧动画 WebP（q36, alpha）→ 内容哈希文件 + 重写 `fx-manifest.ts`，清理旧 webp。
- Python 依赖：`PYTHON` 默认优先 `C:/llm/comfy-venv/Scripts/python.exe`（含 Pillow 12），可用 `PYTHON_BIN` 覆盖。
- `--dry-run`：只打印计划，不联网。
- 生成工作流/提示词：`assets-src/comfyUI_workflow/fx-effects.json`（ComfyUI API 模板）、`assets-src/fx/prompts.json`（脚本实际读取的单一来源）。

## 预算与门禁

- 单文件 ≤120KB、总 ≤1.2MB、边长 128–768、必须多帧且带 alpha；当前 9 个效果 **871KB**。
- `npm run check:fx-assets`：manifest ↔ 文件 ↔ 尺寸/大小/帧数/alpha ↔ 无孤儿文件。

## 运行时行为（详见 ADR-0037）

- 首帧只建 `lastHistorySeq` 基线，不回放旧历史；
- 增量 cue 队列：同目标同类型合并；>12 条只播最近 12 条；总时长 ≤3s；
- 消费期间禁止操作（出牌/技能/选将/确认），允许滚动右侧历史；暂停轮询，结束后刷新并恢复；
- 无跳过按钮；`prefers-reduced-motion: reduce` 退化为描边/文字（≤300ms）；
- 特效素材加载失败 → CSS 光圈兜底，不出现裂图。
