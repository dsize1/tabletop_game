# 拉斯维加斯 · 小游戏素材与映射

> 状态：v1.0（2026-10-19，已验收）。**文件名即映射**，不再需要人工对照表。
> 原图在 `assets-src/las/`（含摄影师水印，**不入库**）；发布物在 `apps/web/public/board/`（git 忽略）。

## 1. 文件命名约定（构建时解析）

| 文件名 | 解析结果 |
|---|---|
| `A1-幸运之拳.jpg` … `H2-我的抉择.jpg` | 资产 id `A1`–`H2`（正则取「字母+数字」，中文名只作可读性） |
| `cover.jpg` | `cover`（页头小图） |
| `board.png` | `board`（六边形底盘贴图） |
| 其它名字 | 回退 `mapping.json` 的 `id`；`id: null` 表示不发布 |

`mapping.json` 只保存**元数据**：`rot`（摆正角度）、`guess`（该块 id 为推测，待实物核对）、`note`。
改文件名后重跑 `node apps/scripts/board-assets.mjs build` 即完成全量对齐。

## 2. 16 块小游戏（引擎名为准）

| id | 引擎名 | 板面英文别名 | id 可信度 |
|---|---|---|---|
| A1 | 幸运之拳 | Lucky Punch / 幸运一击 | 推测（英文吻合） |
| A2 | 大奖挑战 | Play It Big / 黄金时刻 | **待确认** |
| B1 | 主场时间 | Prime Time / 黄金时段 | 确认 |
| B2 | 50% 的机会 | 50/50 | 确认 |
| C1 | 掌声响起 | High Five / 击掌庆祝 | 推测（英文吻合） |
| C2 | 厄运降临 | Bad Luck / 霉运 | 推测（语义吻合） |
| D1 | 发薪愉快 | Pay Day / 发薪日 | 确认 |
| D2 | 权力优势 | Power Play / 数量优势 | **待确认** |
| E1 | 禁止进入 | No Entry | 确认 |
| E2 | 醉倒了吗 | Handycap / 让赛 | **待确认** |
| F1 | 阻止他 | Block It! / 攔截它 | 确认（板面印 `F1`） |
| F2 | 危机就是转机 | Knockdown / 恰似大亨 | **待确认** |
| G1 | 黑色摸彩箱 | Break Box / 黑盒 | 确认 |
| G2 | 双倍颁奖 | Double Down / 双倍下注 | 确认 |
| H1 | 好骰难求 | Nice Dice | 确认 |
| H2 | 我的抉择 | My Choice / 任君选择 | 确认 |

> 引擎译名与板面英文名并非一一对译（F1 板面 `Block It!` ↔ 引擎「阻止他」属正常）。
> 待确认的 4 项以实物玩法为准；确认后把 `mapping.json` 的 `guess` 去掉即可。

## 3. 加工流水线（确定性，无 ML）

1. **去背景**：四边泛洪（木纹种子色 + 邻域容差）→ 取最大连通块（去掉桌上卡牌/阴影）→ 5×5 多数滤波
2. **纠偏**：取中间 50% 行的左边界做线性拟合（±14°）→ 按 `rot` 再转 90/180/270°
3. **紧裁**：透明底裁剪到主体，再等比缩放到宽 **640**（高度随宽高比变化，锚点即图上坐标）
4. **压缩**：WebP q74 + alpha，单张 **13–21KB**

发布结果：16 块 ≈250KB + 封面 10.5KB + 底盘 40KB ≈ **317KB**（预算 900KB / 首屏 280KB）。

## 4. 命令

```bash
node apps/scripts/board-assets.mjs inventory   # 生成 contact sheet（盘点原图）
node apps/scripts/board-assets.mjs build       # 抠图 + 压缩 + 重写 manifest.ts（自动清旧产物）
node apps/scripts/board-assets.mjs preview     # 把发布产物拼一张预览图，人工检查
node apps/scripts/board-assets.mjs build --check-budget   # 超预算即 exit 1
```

产物：`apps/web/public/board/tile-{id}-{hash}.webp`、`cover-{hash}.webp`、`board-hex-{hash}.webp`，清单写入
`apps/web/src/view/board/manifest.ts`（构建产物，随源码提交）。

## 5. 底盘（board）加工记录

| 步骤 | 输入 | 说明 |
|---|---|---|
| 1 | 实物照 | 骰子袋、周围卡牌、台面 |
| 2 | ComfyUI + SD1.5-inpainting（用户执行） | 去骰子袋 |
| 3 | 同上，定向 inpaint 木框塑料膜 | 中间版 |
| 4 | 同上，去右下残膜 | **最终** `board.png` |

- 发布：`board-hex-*.webp`（760×760，≈40KB），裁六边形 + 掏空中央骰盘；开口按木框内沿**外扩 6%**，用来吃掉 AI 残留的黑色六边形与压边残膜。
- 标定：`apps/scripts/board-hex.mjs` 的 `CALIB[文件名]`（`outer` / `inner` / `rot`）。换底图时用 900px 网格图重量一次（可临时生成 `assets-src/derived/board-*-grid.jpg`）。
- 工作流：ComfyUI 遮罩/impaint 编排存于 `assets-src/comfyUI_workflow/遮罩.json`（图文流程见本项目 `docs/modules/board-view.md`）。
- 显示：`USE_BOARD_TEXTURE = true`，`opacity 0.5` 压在卡牌下方；照片自带霓虹店名**保留**（全不透明会与卡牌叠字）。
