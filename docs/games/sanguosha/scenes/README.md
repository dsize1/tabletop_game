# 内核 v2 抽检场景（人工走查用）

> 由 `npm run sgs2:scenes` 生成。**双击 `index.html` 即可在浏览器查看**（自包含，无需服务器）。

每个场景是真实执行新内核（`sanguosha2`）得到的步骤记录：动作前/后 pending、状态（体力/手牌/void/弃牌堆）、日志与展示文案、以及断言是否通过。

| 场景 | 覆盖的裁定 |
|---|---|
| `jushou-flip-jiewei` | 据守翻面=取反；翻回正面**先结算解围②**再摸 4（复验 40） |
| `buqu-chuang-void` | 不屈：判定牌→「创」**移出游戏**（复验 41） |
| `fenji-after-dying` | 奋激：**脱离濒死后**才摸 2（复验 42） |
| `juzhan-two-windows` | 拒战：阳=他人回合内、阴=自己回合内（复验 46） |
| `yili-dying-defer` | 遗礼：代价致濒死时**救回后才给「橘」**（复验 48） |
| `lijun-second-choice` | 立军：交杀后**孙亮二次选择**（复验 49） |

## 命令

```bash
npm run sgs2:scenes        # 重新生成 html + json
npm run check:sgs2:scenes  # CI：场景断言必须全绿
```

## 说明

- 场景只加载**新内核**（`crates/engines/sanguosha2`），不依赖运行中的 server/MCP/Redis，因此可以随时本地复跑。
- 每个 `check` 就是人工走查要确认的点；`✘` 会直接红在页面顶部。
- 场景与 `docs/games/sanguosha/align/<B>.json` 的 `assertions` 一一对应，改裁定要两边一起改。
