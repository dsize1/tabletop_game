# 全武将验证 · 机器证据（自动生成，勿手改）

> 生成：`apps/scripts/sgs-general-audit.mjs` · 时间：2026-11-05T10:33:11.668Z
> 只含**机械证据**（可达/不报错/守恒/pending/可复现），不含语义判定；
> 人工走查结论请写入 `docs/games/sanguosha/general-audit.md`。

- 范围：B0 风林火山·风/林/山（保留）（3 将）
- seed=20261105 · repeat=1 · players=5 · maxSteps=12
- 名录：232 将（审计 148 / 排除 84）/ 295 技能
- 结果：触达 2 · 未触达 1 · 失败 0 · 阻塞 0

| 批次 | 武将 | code | 技能（类型） | 状态 | 触达 | 未触达 | 不变量 | 备注 |
|---|---|---|---|---|---|---|---|---|
| B0 | 曹仁 | sgs_hero_29 | 据守(normal)、解围(normal) | 触达 | jushou | jiewei | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jushou |
| B0 | 周泰 | sgs_hero_210 | 不屈(locked)、奋激(normal) | 触达 | fenji | buqu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：fenji |
| B0 | 曹丕 | sgs_hero_43 | 行殇(normal)、放逐(normal)、颂威(lord) | 未触达 | - | xingshang,fangzhu,songwei | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（xingshang,fangzhu,songwei） |

## 触达率（机器证据，供人工走查排序）

| 维度 | 技能数 | 触达 | 比例 |
|---|---|---|---|
| 批次 B0 | 7 | 2 | 29% |
| 类型 locked | 1 | 0 | 0% |
| 类型 lord | 1 | 0 | 0% |
| 类型 normal | 5 | 2 | 40% |

