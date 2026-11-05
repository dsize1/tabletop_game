# 三国杀 · 全武将验证矩阵（149 将 / 人工验收台账）

> 口径：有界版同名的原版不进批次（用户 2026-11-05 裁定，方案 A）；149 位 = 默认选将池。
> 批次表单一事实源：`apps/scripts/lib/sgs-batches.mjs`（B0–B20）。
> 机器证据：`docs/games/sanguosha/general-audit-evidence.md`（生成，勿手改）+ `data/audit/<batch>.json`。
> **人工列（原文/歧义、人工结果、确认人、备注）只能由人工填写**；agent 不得自行标「人工通过」。

## 1. 状态枚举

- `未验证`：尚无机器证据也无人工走查；
- `自动通过`：机器证据仅证明可达/不报错/守恒/pending 可续/可复现（**不等于语义正确**）；
- `自动失败`：机器证据出现失败（需复现并登记）；
- `待裁定`：技能文本存在歧义，等待用户裁定；
- `人工通过` / `人工失败`：工作台逐项走查后的**唯一验收依据**（确认人签字）；
- `阻塞`：环境/前置缺陷导致无法验证（注明原因）。

## 2. 人工走查记录字段（逐项）

```
武将 / 技能 code / 原文与来源 / 歧义点或裁定 / 期望行为（人工确认）
场景(batch+seed/模板) / 实际操作 / 期望结果 / 实际结果 / 结论(通过/失败/待裁定) / 确认人+日期
```

## 3. 矩阵

| 批次 | 包 | 武将 | code | 技能（类型） | 定向用例 | 原文/歧义 | 机器证据 | 人工结果 | 确认人 | 复现 | 备注 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| B0 | wind_fire_forest_mountain | 曹仁 | `sgs_hero_29` | 据守(`jushou`/normal)、解围(`jiewei`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B0 --seed 20261105` | |
| B0 | wind_fire_forest_mountain | 周泰 | `sgs_hero_210` | 不屈(`buqu`/locked)、奋激(`fenji`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B0 --seed 20261105` | |
| B0 | wind_fire_forest_mountain | 曹丕 | `sgs_hero_43` | 行殇(`xingshang`/normal)、放逐(`fangzhu`/normal)、颂威(`songwei`/lord) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B0 --seed 20261105` | |
| B1 | wind_fire_forest_mountain | 袁术 | `sgs_hero_100` | 庸肆(`yongsi`/locked)、伪帝(`weidi`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B1 --seed 20261105` | |
| B1 | wind_fire_forest_mountain | 王基 | `sgs_hero_362` | 奇制(`qizhi`/normal)、进趋(`jinzou`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B1 --seed 20261105` | |
| B1 | wind_fire_forest_mountain | 严颜 | `sgs_hero_400` | 拒战(`juzhan`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B1 --seed 20261105` | |
| B1 | wind_fire_forest_mountain | 王平 | `sgs_hero_401` | 飞军(`feijun`/normal)、兵略(`binglue`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B1 --seed 20261105` | |
| B1 | wind_fire_forest_mountain | 陆绩 | `sgs_hero_402` | 怀橘(`huaiju`/locked)、遗礼(`yili`/normal)、整论(`zhenglun`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B1 --seed 20261105` | |
| B1 | wind_fire_forest_mountain | 孙亮 | `sgs_hero_403` | 溃诛(`kuizhu`/normal)、掣政(`chezheng`/locked)、立军(`lijun`/lord) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B1 --seed 20261105` | |
| B1 | wind_fire_forest_mountain | 蒯越蒯良 | `sgs_hero_404` | 荐降(`jianxiang`/normal)、审时(`shenshi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B1 --seed 20261105` | |
| B1 | wind_fire_forest_mountain | 许攸 | `sgs_hero_406` | 成略(`chenglue`/normal)、恃才(`shicai`/normal)、寸目(`cunmu`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B1 --seed 20261105` | |
| B2 | wind_fire_forest_mountain | 卢植 | `sgs_hero_407` | 明任(`mingren`/normal)、贞良(`zhenliang`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B2 --seed 20261105` | |
| B2 | wind_fire_forest_mountain | 郝昭 | `sgs_hero_408` | 镇骨(`zhengu`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B2 --seed 20261105` | |
| B2 | wind_fire_forest_mountain | 陈到 | `sgs_hero_409` | 往烈(`wanglie`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B2 --seed 20261105` | |
| B2 | wind_fire_forest_mountain | 诸葛瞻 | `sgs_hero_410` | 罪论(`zuilun`/normal)、父荫(`fuyin`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B2 --seed 20261105` | |
| B2 | wind_fire_forest_mountain | 周妃 | `sgs_hero_411` | 良姻(`liangyin`/normal)、箜声(`kongsheng`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B2 --seed 20261105` | |
| B2 | wind_fire_forest_mountain | 毌丘俭 | `sgs_hero_413` | 征荣(`zhengrong`/normal)、鸿举(`hongju`/awaken) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B2 --seed 20261105` | |
| B2 | wind_fire_forest_mountain | 陆抗 | `sgs_hero_414` | 谦节(`qianjie`/locked)、决堰(`jueyan`/normal)、破势(`poshi`/awaken) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B2 --seed 20261105` | |
| B2 | wind_fire_forest_mountain | 张绣 | `sgs_hero_415` | 雄乱(`xiongluan`/limited)、从谏(`congjian`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B2 --seed 20261105` | |
| B3 | yijiang | 马谡 | `sgs_hero_156` | 散谣(`sanyao`/normal)、制蛮(`zhiman`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B3 --seed 20261105` | |
| B3 | yijiang | 荀攸 | `sgs_hero_164` | 奇策(`qice`/normal)、智愚(`zhiyu`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B3 --seed 20261105` | |
| B3 | yijiang | 将华雄 | `sgs_hero_171` | 恃勇(`shiyong`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B3 --seed 20261105` | |
| B3 | yijiang | 钟会 | `sgs_hero_173` | 权计(`quanji`/normal)、自立(`zili`/awaken) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B3 --seed 20261105` | |
| B3 | yijiang | 满宠 | `sgs_hero_176` | 峻刑(`junxing`/normal)、御策(`yuce`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B3 --seed 20261105` | |
| B3 | yijiang | 关平 | `sgs_hero_177` | 龙吟(`longyin`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B3 --seed 20261105` | |
| B4 | yijiang | 刘封 | `sgs_hero_179` | 陷嗣(`xiansi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B4 --seed 20261105` | |
| B4 | yijiang | 潘璋马忠 | `sgs_hero_180` | 夺刀(`duodao`/normal)、暗箭(`anjian`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B4 --seed 20261105` | |
| B4 | yijiang | 朱然 | `sgs_hero_182` | 胆守(`danshou`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B4 --seed 20261105` | |
| B4 | yijiang | 曹真 | `sgs_hero_321` | 司敌(`sidi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B4 --seed 20261105` | |
| B4 | yijiang | 韩浩史涣 | `sgs_hero_322` | 慎断(`shenduan`/normal)、勇略(`yonglue`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B4 --seed 20261105` | |
| B4 | yijiang | 陈群 | `sgs_hero_323` | 品第(`pindi`/normal)、法恩(`faen`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B4 --seed 20261105` | |
| B4 | yijiang | 吴懿 | `sgs_hero_324` | 奔袭(`benxi`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B4 --seed 20261105` | |
| B4 | yijiang | 周仓 | `sgs_hero_325` | 忠勇(`zhongyong`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B4 --seed 20261105` | |
| B5 | yijiang | 孙鲁班 | `sgs_hero_327` | 谮毁(`zanhui`/normal)、骄矜(`jiaojin`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B5 --seed 20261105` | |
| B5 | yijiang | 朱桓 | `sgs_hero_328` | 奋励(`fenli`/normal)、平寇(`pingkou`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B5 --seed 20261105` | |
| B5 | yijiang | 顾雍 | `sgs_hero_329` | 慎行(`shenxing`/normal)、秉壹(`bingyi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B5 --seed 20261105` | |
| B5 | yijiang | 曹叡 | `sgs_hero_332` | 恢拓(`huituo`/normal)、明鉴(`mingjian`/normal)、兴衰(`xingshuai`/lord) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B5 --seed 20261105` | |
| B5 | yijiang | 曹休 | `sgs_hero_333` | 千驹(`qianju`/locked)、倾袭(`qingxi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B5 --seed 20261105` | |
| B5 | yijiang | 钟繇 | `sgs_hero_334` | 活墨(`huomo`/normal)、佐定(`zuoding`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B5 --seed 20261105` | |
| B6 | yijiang | 刘谌 | `sgs_hero_335` | 战绝(`zhanjue`/normal)、勤王(`qinwang`/lord) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B6 --seed 20261105` | |
| B6 | yijiang | 张嶷 | `sgs_hero_337` | 怃戎(`wurong`/normal)、矢志(`shizhi`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B6 --seed 20261105` | |
| B6 | yijiang | 孙休 | `sgs_hero_338` | 宴诛(`yanzhu`/normal)、兴学(`xingxue`/normal)、诏缚(`zhaofu`/lord) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B6 --seed 20261105` | |
| B6 | yijiang | 朱治 | `sgs_hero_339` | 安国(`anguo`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B6 --seed 20261105` | |
| B6 | yijiang | 全琮 | `sgs_hero_340` | 邀名(`yaoming`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B6 --seed 20261105` | |
| B6 | yijiang | 公孙渊 | `sgs_hero_341` | 怀异(`huaiyi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B6 --seed 20261105` | |
| B6 | yijiang | 郭图逢纪 | `sgs_hero_342` | 急攻(`jigong`/normal)、饰非(`shifei`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B6 --seed 20261105` | |
| B7 | yijiang | 李严 | `sgs_hero_381` | 督粮(`duliang`/normal)、腹鳞(`fulin`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B7 --seed 20261105` | |
| B7 | yijiang | 孙登 | `sgs_hero_382` | 匡弼(`kuangbi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B7 --seed 20261105` | |
| B7 | yijiang | 刘虞 | `sgs_hero_383` | 止戈(`zhige`/normal)、宗祚(`zongzuo`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B7 --seed 20261105` | |
| B7 | yijiang | 岑昏 | `sgs_hero_384` | 极奢(`jishe`/normal)、链祸(`lianhuo`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B7 --seed 20261105` | |
| B7 | yijiang | 孙资刘放 | `sgs_hero_385` | 瑰藻(`guizao`/normal)、讥谀(`jiyu`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B7 --seed 20261105` | |
| B7 | yijiang | 黄皓 | `sgs_hero_386` | 寝情(`qinqing`/normal)、贿生(`huisheng`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B7 --seed 20261105` | |
| B7 | yijiang | 张让 | `sgs_hero_387` | 滔乱(`taoluan`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B7 --seed 20261105` | |
| B8 | yijiang | 辛宪英 | `sgs_hero_388` | 忠鉴(`zhongjian`/normal)、才识(`caishi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B8 --seed 20261105` | |
| B8 | yijiang | 吴苋 | `sgs_hero_389` | 福绵(`fumian`/normal)、怠宴(`daiyan`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B8 --seed 20261105` | |
| B8 | yijiang | 徐氏 | `sgs_hero_390` | 问卦(`wengua`/normal)、伏诛(`fuzhu`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B8 --seed 20261105` | |
| B8 | yijiang | 曹节 | `sgs_hero_391` | 守玺(`shouxi`/normal)、惠民(`huimin`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B8 --seed 20261105` | |
| B8 | yijiang | 嵇康 | `sgs_hero_392` | 清弦(`qingxian`/normal)、绝响(`juexiang`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B8 --seed 20261105` | |
| B8 | yijiang | 秦宓 | `sgs_hero_393` | 谏征(`jianzheng`/normal)、专对(`zhuandui`/normal)、天辩(`tianbian`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B8 --seed 20261105` | |
| B8 | yijiang | 薛综 | `sgs_hero_394` | 复难(`funan`/normal)、诫训(`jiexun`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B8 --seed 20261105` | |
| B8 | yijiang | 蔡邕 | `sgs_hero_395` | 辟撰(`bizhuan`/normal)、通博(`tongbo`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B8 --seed 20261105` | |
| B9 | jiexian | 界刘备 | `sgs_hero_299` | 仁德(`rende`/normal)、激将(`jijiang`/lord) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B9 --seed 20261105` | |
| B9 | jiexian | 界关羽 | `sgs_hero_300` | 义绝(`yijue`/normal)、武圣(`wusheng`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B9 --seed 20261105` | |
| B9 | jiexian | 界张飞 | `sgs_hero_301` | 替身(`tishen`/limited)、咆哮(`paoxiao`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B9 --seed 20261105` | |
| B9 | jiexian | 界赵云 | `sgs_hero_302` | 涯角(`yajiao`/normal)、龙胆(`longdan`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B9 --seed 20261105` | |
| B9 | jiexian | 界马超 | `sgs_hero_303` | 马术(`mashu`/locked)、铁骑(`tieqi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B9 --seed 20261105` | |
| B9 | jiexian | 界诸葛亮 | `sgs_hero_440` | 空城(`kongcheng`/locked)、观星(`guanxing`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B9 --seed 20261105` | |
| B9 | jiexian | 界黄月英 | `sgs_hero_441` | 集智(`jizhi`/normal)、奇才(`qicai`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B9 --seed 20261105` | |
| B10 | jiexian | 界甘宁 | `sgs_hero_305` | 奇袭(`qixi`/normal)、奋威(`fenwei`/limited) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B10 --seed 20261105` | |
| B10 | jiexian | 界吕蒙 | `sgs_hero_306` | 克己(`keji`/normal)、勤学(`qinxue`/awaken)、博图(`botu`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B10 --seed 20261105` | |
| B10 | jiexian | 界黄盖 | `sgs_hero_307` | 苦肉(`kurou`/normal)、诈降(`zhaxiang`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B10 --seed 20261105` | |
| B10 | jiexian | 界周瑜 | `sgs_hero_308` | 英姿(`yingzi`/locked)、反间(`fanjian`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B10 --seed 20261105` | |
| B10 | jiexian | 界大乔 | `sgs_hero_309` | 流离(`liuli`/normal)、国色(`guose`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B10 --seed 20261105` | |
| B10 | jiexian | 界陆逊 | `sgs_hero_310` | 谦逊(`qianxun`/normal)、连营(`lianying`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B10 --seed 20261105` | |
| B10 | jiexian | 界孙权 | `sgs_hero_442` | 制衡(`zhiheng`/normal)、救援(`jiuyuan`/lord) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B10 --seed 20261105` | |
| B10 | jiexian | 界孙尚香 | `sgs_hero_443` | 枭姬(`xiaoji`/normal)、结姻(`jieyin`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B10 --seed 20261105` | |
| B11 | jiexian | 界曹操 | `sgs_hero_311` | 奸雄(`jianxiong`/normal)、护驾(`hujia`/lord) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B11 --seed 20261105` | |
| B11 | jiexian | 界司马懿 | `sgs_hero_312` | 反馈(`fankui`/normal)、鬼才(`guicai`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B11 --seed 20261105` | |
| B11 | jiexian | 界夏侯惇 | `sgs_hero_313` | 刚烈(`ganglie`/normal)、清俭(`qingjian`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B11 --seed 20261105` | |
| B11 | jiexian | 界张辽 | `sgs_hero_314` | 突袭(`tuxi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B11 --seed 20261105` | |
| B11 | jiexian | 界许褚 | `sgs_hero_315` | 裸衣(`luoyi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B11 --seed 20261105` | |
| B11 | jiexian | 界郭嘉 | `sgs_hero_316` | 天妒(`tiandu`/normal)、遗计(`yiji`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B11 --seed 20261105` | |
| B11 | jiexian | 界李典 | `sgs_hero_317` | 恂恂(`xunxun`/normal)、忘隙(`wangxi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B11 --seed 20261105` | |
| B11 | jiexian | 界甄姬 | `sgs_hero_444` | 倾国(`qingguo`/normal)、洛神(`luoshen`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B11 --seed 20261105` | |
| B12 | jiexian | 界华佗 | `sgs_hero_318` | 急救(`jijiu`/normal)、除疠(`chuli`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B12 --seed 20261105` | |
| B12 | jiexian | 界吕布 | `sgs_hero_319` | 无双(`wushuang`/locked)、利驭(`liyu`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B12 --seed 20261105` | |
| B12 | jiexian | 界公孙瓒 | `sgs_hero_320` | 义从(`yicong`/locked)、趫猛(`qiaomeng`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B12 --seed 20261105` | |
| B12 | jiexian | 界貂蝉 | `sgs_hero_445` | 离间(`lijian`/normal)、闭月(`biyue`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B12 --seed 20261105` | |
| B13 | jiexian | 界黄忠 | `sgs_hero_26` | 烈弓(`liegong`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B13 --seed 20261105` | |
| B13 | jiexian | 界徐庶 | `sgs_hero_304` | 诛害(`zhuhai`/normal)、潜心(`qianxin`/awaken) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B13 --seed 20261105` | |
| B13 | jiexian | 界华雄 | `sgs_hero_446` | 耀武(`yaowu`/locked)、势斩(`shizhan`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B13 --seed 20261105` | |
| B13 | jiexian | 界左慈 | `sgs_hero_447` | 化身(`huashen`/normal)、新生(`xinsheng`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B13 --seed 20261105` | |
| B13 | jiexian | 界张角 | `sgs_hero_448` | 雷击(`leiji`/normal)、鬼道(`guidao`/normal)、黄天(`huangtian`/lord) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B13 --seed 20261105` | |
| B13 | jiexian | 界于吉 | `sgs_hero_449` | 蛊惑(`guhuo`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B13 --seed 20261105` | |
| B13 | jiexian | 界袁绍 | `sgs_hero_450` | 乱击(`luanji`/normal)、血裔(`xueyi`/lord) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B13 --seed 20261105` | |
| B13 | jiexian | 界刘禅 | `sgs_hero_451` | 享乐(`xiangle`/locked)、放权(`fangquan`/normal)、若愚(`ruoyu`/lord) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B13 --seed 20261105` | |
| B14 | jiexian | 界孙策 | `sgs_hero_452` | 制霸(`zhiba`/lord)、激昂(`jiang`/normal)、魂姿(`hunzi`/awaken) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B14 --seed 20261105` | |
| B14 | jiexian | 界夏侯渊 | `sgs_hero_453` | 神速(`shensu`/normal)、设变(`shebian`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B14 --seed 20261105` | |
| B14 | jiexian | 界卧龙诸葛亮 | `sgs_hero_454` | 八阵(`bazhen`/locked)、藏拙(`cangzhuo`/locked)、火计(`huoji`/normal)、看破(`kanpo`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B14 --seed 20261105` | |
| B14 | jiexian | 界庞统 | `sgs_hero_455` | 连环(`lianhuan`/normal)、涅槃(`niepan`/limited) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B14 --seed 20261105` | |
| B14 | jiexian | 界魏延 | `sgs_hero_456` | 狂骨(`kuanggu`/normal)、奇谋(`qimou`/limited) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B14 --seed 20261105` | |
| B14 | jiexian | 界小乔 | `sgs_hero_457` | 红颜(`hongyan`/locked)、天香(`tianxiang`/normal)、飘零(`piaoling`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B14 --seed 20261105` | |
| B14 | jiexian | 界孙坚 | `sgs_hero_458` | 英魂(`yinghun`/normal)、武烈(`wulie`/limited) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B14 --seed 20261105` | |
| B14 | jiexian | 界庞德 | `sgs_hero_462` | 马术(`mashu`/locked)、鞬出(`jiandu`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B14 --seed 20261105` | |
| B15 | jiexian | 界太史慈 | `sgs_hero_463` | 天义(`tianyi`/normal)、酣战(`hanzhan`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B15 --seed 20261105` | |
| B15 | jiexian | 界董卓 | `sgs_hero_470` | 肉林(`roulin`/locked)、崩坏(`bengkuai`/locked)、酒池(`jiuchi`/normal)、暴虐(`baonue`/lord) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B15 --seed 20261105` | |
| B15 | jiexian | 界邓艾 | `sgs_hero_471` | 屯田(`tuotian`/normal)、凿险(`zaoxian`/awaken) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B15 --seed 20261105` | |
| B15 | jiexian | 界祝融 | `sgs_hero_479` | 巨象(`juxiang`/locked)、烈刃(`liereng`/normal)、长标(`changbiao`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B15 --seed 20261105` | |
| B15 | jiexian | 界姜维 | `sgs_hero_480` | 挑衅(`tiaoxin`/normal)、志继(`zhiji`/awaken) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B15 --seed 20261105` | |
| B15 | jiexian | 界贾诩 | `sgs_hero_484` | 帷幕(`weimu`/locked)、乱武(`luanwu`/limited)、完杀(`wansha`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B15 --seed 20261105` | |
| B15 | jiexian | 界鲁肃 | `sgs_hero_485` | 好施(`haoshi`/normal)、缔盟(`dimeng`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B15 --seed 20261105` | |
| B15 | jiexian | 界张郃 | `sgs_hero_486` | 巧变(`qiaobian`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B15 --seed 20261105` | |
| B16 | jiexian | 界孟获 | `sgs_hero_492` | 祸首(`huoshou`/locked)、再起(`zaizhi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B16 --seed 20261105` | |
| B16 | jiexian | 界徐晃 | `sgs_hero_496` | 断粮(`duanliang`/normal)、截辎(`jiezi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B16 --seed 20261105` | |
| B16 | jiexian | 界蔡文姬 | `sgs_hero_505` | 断肠(`duanchang`/locked)、悲歌(`beige`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B16 --seed 20261105` | |
| B16 | jiexian | 界荀彧 | `sgs_hero_515` | 驱虎(`quhu`/normal)、节命(`jieming`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B16 --seed 20261105` | |
| B16 | jiexian | 界典韦 | `sgs_hero_526` | 强袭(`qiangxi`/normal)、狞恶(`ninge`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B16 --seed 20261105` | |
| B16 | jiexian | 界颜良文丑 | `sgs_hero_550` | 双雄(`shuangxiong`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B16 --seed 20261105` | |
| B16 | jiexian | 界张昭张纮 | `sgs_hero_563` | 直谏(`zhijian`/normal)、固政(`guzheng`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B16 --seed 20261105` | |
| B16 | jiexian | 界徐盛 | `sgs_hero_587` | 破军(`pojun`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B16 --seed 20261105` | |
| B17 | jiexian | 界凌统 | `sgs_hero_588` | 旋风(`xuanfeng`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B17 --seed 20261105` | |
| B17 | jiexian | 界陈宫 | `sgs_hero_597` | 智迟(`zhichi`/locked)、明策(`mingce`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B17 --seed 20261105` | |
| B17 | jiexian | 界曹彰 | `sgs_hero_599` | 将驰(`jiangchi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B17 --seed 20261105` | |
| B17 | jiexian | 界简雍 | `sgs_hero_600` | 纵适(`zongshi`/normal)、巧说(`qiaoshuo`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B17 --seed 20261105` | |
| B17 | jiexian | 界虞翻 | `sgs_hero_603` | 纵玄(`zongxuan`/normal)、直言(`zhiyan`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B17 --seed 20261105` | |
| B17 | jiexian | 界高顺 | `sgs_hero_604` | 陷阵(`xianzhen`/normal)、禁酒(`jinjiu`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B17 --seed 20261105` | |
| B17 | jiexian | 界法正 | `sgs_hero_610` | 眩惑(`xuanhuo`/normal)、恩怨(`enyuan`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B17 --seed 20261105` | |
| B17 | jiexian | 界程普 | `sgs_hero_620` | 疠火(`lihuo`/normal)、醇醪(`chunlao`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B17 --seed 20261105` | |
| B18 | jiexian | 界王异 | `sgs_hero_621` | 贞烈(`zhenlie`/normal)、秘计(`miji`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B18 --seed 20261105` | |
| B18 | jiexian | 界张春华 | `sgs_hero_625` | 绝情(`jueqing`/locked)、伤逝(`shangshi`/normal)、翦灭(`jianmie`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B18 --seed 20261105` | |
| B18 | jiexian | 界刘表 | `sgs_hero_626` | 自守(`zishou`/normal)、宗室(`zongshi`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B18 --seed 20261105` | |
| B18 | jiexian | 界蔡夫人 | `sgs_hero_627` | 献州(`xianzhou`/limited)、窃听(`qieting`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B18 --seed 20261105` | |
| B18 | jiexian | 界曹冲 | `sgs_hero_628` | 称象(`chengxiang`/normal)、仁心(`renxin`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B18 --seed 20261105` | |
| B18 | jiexian | 界曹植 | `sgs_hero_629` | 落英(`luoying`/normal)、酒诗(`jiushi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B18 --seed 20261105` | |
| B18 | jiexian | 界吴国太 | `sgs_hero_640` | 甘露(`ganlu`/normal)、补益(`buyi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B18 --seed 20261105` | |
| B18 | jiexian | 界廖化 | `sgs_hero_641` | 当先(`dangxian`/normal)、伏枥(`fuli`/limited) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B18 --seed 20261105` | |
| B19 | jiexian | 界李儒 | `sgs_hero_642` | 灭计(`mieji`/normal)、焚城(`fencheng`/limited)、绝策(`juece`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B19 --seed 20261105` | |
| B19 | jiexian | 界郭淮 | `sgs_hero_658` | 精策(`jingce`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B19 --seed 20261105` | |
| B19 | jiexian | 界伏皇后 | `sgs_hero_671` | 惴恐(`zhuikong`/normal)、求援(`qiuyuan`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B19 --seed 20261105` | |
| B19 | jiexian | 界韩当 | `sgs_hero_676` | 弓骑(`gongqi`/normal)、解烦(`jiefan`/limited) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B19 --seed 20261105` | |
| B19 | jiexian | 界关兴张苞 | `sgs_hero_690` | 父魂(`fuhen`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B19 --seed 20261105` | |
| B19 | jiexian | 界马岱 | `sgs_hero_696` | 马术(`mashu`/locked)、潜袭(`qianxi`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B19 --seed 20261105` | |
| B19 | jiexian | 界郭皇后 | `sgs_hero_726` | 矫诏(`jiazhao`/normal)、殚心(`danxin`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B19 --seed 20261105` | |
| B19 | jiexian | 界沮授 | `sgs_hero_749` | 矢北(`shibei`/locked)、渐营(`jianying`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B19 --seed 20261105` | |
| B20 | jiexian | 界夏侯氏 | `sgs_hero_752` | 樵拾(`qiaoshi`/normal)、燕语(`yanyu`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B20 --seed 20261105` | |
| B20 | jiexian | 界张松 | `sgs_hero_753` | 强识(`qiangshi`/normal)、献图(`xiantu`/normal) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B20 --seed 20261105` | |
| B20 | jiexian | 界于禁 | `sgs_hero_756` | 镇军(`zhenjun`/normal)、毅重(`yizhong`/locked) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B20 --seed 20261105` | |
| B20 | jiexian | 界步练师 | `sgs_hero_775` | 安恤(`anxu`/normal)、追忆(`zhuixi`/limited) | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | `--batch B20 --seed 20261105` | |

## 4. 未纳入批次的武将（83 位，有界版同名）

按口径 A 排除，验证由界版覆盖同名技能（如 `界刘备` 覆盖 `刘备`）：

- **standard**（25）：刘备→sgs_hero_299、关羽→sgs_hero_300、张飞→sgs_hero_301、诸葛亮→sgs_hero_440、赵云→sgs_hero_302、马超→sgs_hero_303、黄月英→sgs_hero_441、孙权→sgs_hero_442、甘宁→sgs_hero_305、吕蒙→sgs_hero_306、黄盖→sgs_hero_307、周瑜→sgs_hero_308、大乔→sgs_hero_309、陆逊→sgs_hero_310、曹操→sgs_hero_311、司马懿→sgs_hero_312、夏侯惇→sgs_hero_313、张辽→sgs_hero_314、许褚→sgs_hero_315、郭嘉→sgs_hero_316、甄姬→sgs_hero_444、华佗→sgs_hero_318、吕布→sgs_hero_319、貂蝉→sgs_hero_445、孙尚香→sgs_hero_443
- **wind_fire_forest_mountain**（29）：魏延→sgs_hero_456、夏侯渊→sgs_hero_453、小乔→sgs_hero_457、典韦→sgs_hero_526、荀彧→sgs_hero_515、庞统→sgs_hero_455、卧龙诸葛亮→sgs_hero_454、太史慈→sgs_hero_463、庞德→sgs_hero_462、颜良文丑→sgs_hero_550、袁绍→sgs_hero_450、徐晃→sgs_hero_496、孙坚→sgs_hero_458、董卓→sgs_hero_470、祝融→sgs_hero_479、孟获→sgs_hero_492、贾诩→sgs_hero_484、鲁肃→sgs_hero_485、张郃→sgs_hero_486、邓艾→sgs_hero_471、姜维→sgs_hero_480、刘禅→sgs_hero_451、孙策→sgs_hero_452、张昭张纮→sgs_hero_563、左慈→sgs_hero_447、蔡文姬→sgs_hero_505、黄忠→sgs_hero_26、于吉→sgs_hero_449、张角→sgs_hero_448
- **yijiang**（30）：曹植→sgs_hero_629、高顺→sgs_hero_604、陈宫→sgs_hero_597、凌统→sgs_hero_588、吴国太→sgs_hero_640、徐盛→sgs_hero_587、于禁→sgs_hero_756、张春华→sgs_hero_625、曹彰→sgs_hero_599、廖化→sgs_hero_641、马岱→sgs_hero_696、步练师→sgs_hero_775、程普→sgs_hero_620、韩当→sgs_hero_676、刘表→sgs_hero_626、曹冲→sgs_hero_628、郭淮→sgs_hero_658、简雍→sgs_hero_600、虞翻→sgs_hero_603、伏皇后→sgs_hero_671、李儒→sgs_hero_642、法正→sgs_hero_610、徐庶→sgs_hero_304、王异→sgs_hero_621、关兴张苞→sgs_hero_690、张松→sgs_hero_753、沮授→sgs_hero_749、蔡夫人→sgs_hero_627、夏侯氏→sgs_hero_752、郭皇后→sgs_hero_726

