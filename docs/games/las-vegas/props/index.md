# 拉斯维加斯（Las Vegas Royale）道具总览

> 状态：草案 v0.2（据用户裁定规则重写）。道具按「公共目录 → 游戏 → 道具类型」三级文件路径分类，道具间再细分。
> 规则依据：`rules.md`（用户 2026-10-13 裁定文本）。

## 目录结构（相对 `docs/games/las-vegas/`）

```
las-vegas/
  rules.md               # 规则文档
  props/
    index.md             # 本文件：道具总览 + 路径分类约定
    dice.md              # 骰子（小/大/灰/黑，按颜色与用途细分）
    casinos.md           # 赌场牌（1–6 号）
    banknotes.md         # 钞票（按面额细分）
    chips.md             # 筹码
    small-games.md       # 小游戏板块（A1–H2 十六块）
    markers.md           # 标记（大奖/特权/禁止/箭头/黑色摸彩/指示物）
  legal-actions.md
  data-structure.md
  state-machine.md
  rule-engine-permission.md
```

## 组件清单

| 道具 | 数量 | 说明 | 详见 |
|---|---|---|---|
| 小骰 | 每人 7 枚（同色） | 每轮投掷并下注 | props/dice.md |
| 大骰 | 每人 1 枚（同色） | 钞票分配时算 2 小骰；酒吧算 1 | props/dice.md |
| 灰色骰 | 视 F1/F2 小游戏而定 | 视为虚拟玩家，干扰移除平手/名次 | props/dice.md |
| 黑色骰 | 触发类小游戏时投掷（每次 2 颗） | A2/B1/B2/H2 | props/dice.md |
| 赌场牌 | 6 块（1–6 号） | 骰子落点 + 钞票堆叠 | props/casinos.md |
| 钞票 | 每轮 6 组 × 2 张 | 各赌场奖池，结算前隐藏 | props/banknotes.md |
| 筹码 | 初始每人 2，每轮 +2 | 计分（×$1 万）+ 支付弃回合/小游戏 | props/chips.md |
| 小游戏板块 | 16 块（A1–H2），设置阶段开局人选 3–6 | 拼在 1–3 号赌场下 | props/small-games.md |
| 标记 | 大奖/特权/禁止/箭头/黑色摸彩/指示物 | 各小游戏状态 | props/markers.md |

## 路径分类约定

- 根：`docs/games/<game_id>/`（game_id = `las-vegas`）。
- 规则：`rules.md`。
- 道具：`props/` 下按**道具类型**建文件；单类型内部再按**颜色/编号/面额/用途**细分小节。
- 扩展（若含 Boulevard 等）在 `props/` 或 `extensions/` 下扩展，不污染主规则文件。

> 【待确认】「公共目录」根路径最终落在哪（`docs/games/` / web 静态 `public/` / server 资源目录），以及道具是否需配图片资产（来源与版权）。
