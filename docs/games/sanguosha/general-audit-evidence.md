# 全武将验证 · 机器证据（自动生成，勿手改）

> 生成：`apps/scripts/sgs-general-audit.mjs` · 时间：2026-11-05T14:35:11.219Z
> 只含**机械证据**（可达/不报错/守恒/pending/可复现），不含语义判定；
> 人工走查结论请写入 `docs/games/sanguosha/general-audit.md`。

- 范围：B2 风林火山·SP-2（8 将）
- seed=20261105 · repeat=2 · players=5 · maxSteps=12
- 名录：232 将（审计 148 / 排除 84）/ 295 技能
- 结果：触达 6 · 未触达 2 · 失败 0 · 阻塞 0

| 批次 | 武将 | code | 技能（类型） | 状态 | 触达 | 未触达 | 不变量 | 备注 |
|---|---|---|---|---|---|---|---|---|
| B2 | 卢植 | sgs_hero_407 | 明任(normal)、贞良(normal) | 未触达 | - | mingren,zhenliang | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（mingren,zhenliang） |
| B2 | 郝昭 | sgs_hero_408 | 镇骨(normal) | 触达 | zhengu | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zhengu |
| B2 | 陈到 | sgs_hero_409 | 往烈(normal) | 触达 | wanglie | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：wanglie |
| B2 | 诸葛瞻 | sgs_hero_410 | 罪论(normal)、父荫(locked) | 触达 | zuilun | fuyin | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zuilun |
| B2 | 周妃 | sgs_hero_411 | 良姻(normal)、箜声(normal) | 触达 | kongsheng | liangyin | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：kongsheng |
| B2 | 毌丘俭 | sgs_hero_413 | 征荣(normal)、鸿举(awaken) | 未触达 | - | zhengrong,hongju | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（zhengrong,hongju） |
| B2 | 陆抗 | sgs_hero_414 | 谦节(locked)、决堰(normal)、破势(awaken) | 触达 | jueyan | qianjie,poshi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jueyan |
| B2 | 张绣 | sgs_hero_415 | 雄乱(limited)、从谏(normal) | 触达 | xiongluan | congjian | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：xiongluan |

## 触达率（机器证据，供人工走查排序）

| 维度 | 技能数 | 触达 | 比例 |
|---|---|---|---|
| 批次 B2 | 15 | 6 | 40% |
| 类型 awaken | 2 | 0 | 0% |
| 类型 limited | 1 | 1 | 100% |
| 类型 locked | 2 | 0 | 0% |
| 类型 normal | 10 | 5 | 50% |

