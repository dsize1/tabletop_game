# 全武将验证 · 机器证据（自动生成，勿手改）

> 生成：`apps/scripts/sgs-general-audit.mjs` · 时间：2026-11-11T03:19:43.537Z
> 只含**机械证据**（可达/不报错/守恒/pending/可复现），不含语义判定；
> 人工走查结论请写入 `docs/games/sanguosha/general-audit.md`。

- 范围：B20 界限突破·J8（4 将）
- seed=20261105 · repeat=2 · players=5 · maxSteps=12
- 名录：232 将（审计 147 / 排除 85）/ 288 技能
- 结果：触达 4 · 未触达 0 · 失败 0 · 阻塞 0

| 批次 | 武将 | code | 技能（类型） | 状态 | 触达 | 未触达 | 不变量 | 备注 |
|---|---|---|---|---|---|---|---|---|
| B20 | 界夏侯氏 | sgs_hero_752 | 樵拾(normal)、燕语(normal) | 触达 | yanyu | qiaoshi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：yanyu |
| B20 | 界张松 | sgs_hero_753 | 强识(normal)、献图(normal) | 触达 | qiangshi | xiantu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：qiangshi |
| B20 | 界于禁 | sgs_hero_756 | 镇军(normal)、毅重(normal) | 触达 | zhenjun | yizhong | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zhenjun |
| B20 | 界步练师 | sgs_hero_775 | 安恤(normal)、追忆(normal) | 触达 | anxu | zhuixi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：anxu |

## 触达率（机器证据，供人工走查排序）

| 维度 | 技能数 | 触达 | 比例 |
|---|---|---|---|
| 批次 B20 | 8 | 4 | 50% |
| 类型 normal | 8 | 4 | 50% |

