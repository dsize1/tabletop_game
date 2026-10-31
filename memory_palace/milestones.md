# 里程碑记录（开发进度）

> 记录项目关键节点与验证结果，按时间倒序。重大技术决策见 `decisions/`。

## 🔖 续接指针（下次会话说「继续」时从这里开工）

- **入口**：`docs/games/sanguosha/development-plan.md` → **P4.4.3 已验收（包开关全量预设 + 全套门禁）** → 下一步 P4.5（武将流程工作台）/ 其他批次
- **顺序**：① 界限突破 87/87 已完成 → ② P4.4.3 包开关/全套门禁验收 ✅ → ③ 待定（P5 工作台 / 新包）
- **开工命令**：`npm run sgs:batch-status`（看下一批名单/计数一致性）；流程与模板见 `docs/games/sanguosha/batch-playbook.md`
- **批前必读**：`skills.md` §2.8.12、`rules-rulings.md` §37、`docs/conventions/coding-standards.md`
- **每批收尾**：`p67` 池断言（如涉及界版池）；`skills.md` §2.8.x + `rules-rulings.md` 新节；`milestones.md` 新增 M 条目
- **门禁**：`npm run sgs:check`（每批收尾）、`npm run sgs:check:full`（含 MCP 冒烟）、`npm run sgs:check:fast`（改码自检）

## M56：三国杀 P4.4.3 验收（包开关全量预设 + 全套门禁）✅

- **日期**：2026-10-31
- **完成**：lobby 武将包预设由 5 个手工组合扩为**全量 8 组合**（`general_pack_presets`），标签动态生成；可用人数 25 / 73 / 97 / 87 / 145 / 107 / 129 / 149（去重后）
- **默认口径**：`Config.general_packs` 仍默认 `["standard"]`；旧局/旧快照不回溯追加包
- **回归用例**：`tests/p67.rs` 新增 4 条（预设↔标签↔池人数、大厅预设列表、疆界池候选去重、旧对局快照固定）
- **门禁**：`cargo test -p sanguosha` **874** 全绿；`sgs:check` / `sgs:check:full`（含 MCP 冒烟）全绿；MCP 冒烟新增 `standard+jiexian` 断言
- **文档**：`data-model.md` §3.7、`legal-actions.md` §3、`skills.md` §2.8/§4、`data/README.md` 同步
- **详见**：`memory_palace/plan/01a1931e-2adb-7668-91f6-e77ed68dc2b6.md`

## M55：三国杀 P4.4.2 批次 J8（界限突破 752/753/756/775：4 将 / 8 技能，界限突破 87/87 收官）✅

- **日期**：2026-10-31
- **完成**：界夏侯氏（樵拾/燕语）、界张松（强识/献图）、界于禁（镇军/毅重）、界步练师（安恤/追忆）
- **机制**：界樵拾（任意结束阶段 + 本轮失效）、界献图（伤害 < 交牌数失体力）、界镇军（结束阶段延后摸牌）、毅重（黑杀免疫 / 黑杀不可响应）、界追忆（结束阶段发动）
- **用例**：`tests/p103.rs` 12 条；`cargo test -p sanguosha` **870** 全绿；`p67` 池断言 83 → 87
- **已知边界**：见 `rules-rulings.md` §37
- **详见**：`memory_palace/plan/01a192cd-9c3e-7668-91f6-e77ceff1a5f9.md`

## M54：三国杀 P4.4.2 批次 J7（界限突破 642/658/671/676/690/696/726/749：8 将 / 15 技能）✅

- **日期**：2026-10-31
- **完成**：界李儒（灭计/焚城/绝策）、界郭淮（精策）、界伏皇后（惴恐/求援）、界韩当（弓骑/解烦）、界关兴张苞（父魂）、界马岱（马术/潜袭）、界郭皇后（矫诏/殚心）、界沮授（矢北/渐营）
- **机制**：界绝策（目标手牌 ≤ 你）、界灭计（任意锦囊 + 任意两张弃牌）、界解烦（第一轮不记已发动）；其余界版强化登记为边界
- **用例**：`tests/p102.rs` 24 条；`cargo test -p sanguosha` **858** 全绿；`p67` 池断言 75 → 83
- **已知边界**：见 `rules-rulings.md` §36
- **详见**：`memory_palace/plan/01a192cd-9c3e-7668-91f6-e77ceff1a5f9.md`

## M53：三国杀 P4.4.2 批次 J6（界限突破 621/625/626/627/628/629/640/641：8 将 / 17 技能）✅

- **日期**：2026-10-31
- **完成**：界王异（贞烈/秘计）、界张春华（绝情/伤逝/翦灭）、界刘表（自守/宗室）、界蔡夫人（献州/窃听）、界曹冲（称象/仁心）、界曹植（落英/酒诗）、界吴国太（甘露/补益）、界廖化（当先/伏枥）
- **机制**：`resolve_jiexian7.rs`（`Frame::ZhenLieJie` / `JianMie`、`zongshi_guard`、`dangxian_jie_*`）；`miji_extra_times`、`chengxiang_bonus`、`zongshi_guard:<kingdom>`、`dangxian_jie_no_dmg`
- **用例**：`tests/p101.rs` 25 条；`cargo test -p sanguosha` **833** 全绿；`p67` 池断言 67 → 75
- **已知边界**：见 `rules-rulings.md` §35
- **详见**：`memory_palace/plan/01a192cd-9c3e-7668-91f6-e77ceff1a5f9.md`

## M52：三国杀 P4.4.2 批次 J5（界限突破 588/597/599/600/603/604/610/620：8 将 / 14 技能）✅

- **日期**：2026-10-31
- **完成**：界凌统（旋风）、界陈宫（智迟/明策）、界曹彰（将驰）、界简雍（纵适/巧说）、界虞翻（纵玄/直言）、界高顺（陷阵/禁酒）、界法正（眩惑/恩怨）、界程普（疠火/醇醪）
- **机制**：`resolve_jiexian6.rs`（相邻/上家判定、`chunlao_place`、`mingce_on_damage`、`lihuo_jie_cost`、`zhiyan_upper_trigger`、`jinjiu_blocks_jiu`）；`lost_by:<seat>`；`jiangchi_no_ban` / `sha_limit_penalty`；`mingce_sha:<owner>`
- **用例**：`tests/p100.rs` 24 条；`cargo test -p sanguosha` **809** 全绿；`p67` 池断言 59 → 67
- **已知边界**：见 `rules-rulings.md` §34
- **详见**：`memory_palace/plan/01a192cd-9c3e-7668-91f6-e77ceff1a5f9.md`

## M51：三国杀 P4.4.2 批次 J4（界限突破 492/496/505/515/526/550/563/587：8 将 / 14 技能）✅

- **日期**：2026-10-31
- **完成**：界孟获（祸首/再起）、界徐晃（断粮/截辎）、界蔡文姬（断肠/悲歌）、界荀彧（驱虎/节命）、界典韦（强袭/狞恶）、界颜良文丑（双雄）、界张昭张纮（直谏/固政）、界徐盛（破军）
- **机制**：`Frame::ZaiQiJie` / `JieZiJie` / `GuZhengJie`（`resolve_jiexian5.rs`）；`red_discarded`；`mark_counts["zai"]`；`dmg_count:<seat>`（狞恶）；`sx_dmg:<card>`（双雄）；`discarded_by:<seat>` + `gz_jie:<seat>:<card>`（固政）；`pojun_damage_bonus`
- **用例**：`tests/p99.rs` 25 条；`cargo test -p sanguosha` **785** 全绿；`p67` 池断言 51 → 59
- **已知边界**：见 `rules-rulings.md` §33
- **详见**：`memory_palace/plan/01a192cd-9c3e-7668-91f6-e77ceff1a5f9.md`

## M50：三国杀 P4.4.2 批次 J3（界限突破 463/470/471/479/480/484/485/486：8 将 / 19 技能）✅

- **日期**：2026-10-31
- **完成**：界太史慈（天义/酣战）、界董卓（肉林/崩坏/酒池/暴虐）、界邓艾（屯田/凿险）、界祝融（巨象/烈刃/长标）、界姜维（挑衅/志继）、界贾诩（帷幕/乱武/完杀）、界鲁肃（好施/缔盟）、界张郃（巧变）
- **机制**：`Frame::TiaoxinWait`（`resolve_jiexian4.rs`）；酣战拼点随机 + 获得最大点数【杀】；`jiu_unlimited` / `bengkuai_off`；`changbiao_sha:<card>` + 阶段末摸牌；帷幕回合内防伤摸 2 倍；乱武结算后无距离【杀】；好施回礼（`haoshi_back:<receiver>`）；巧变「变」标记（`mark_counts["bian"]` + `qjb_end:<n>`）
- **用例**：`tests/p98.rs` 24 条；`cargo test -p sanguosha` **760** 全绿；`p67` 池断言 43 → 51
- **已知边界**：见 `rules-rulings.md` §32
- **详见**：`memory_palace/plan/01a192cd-9c3e-7668-91f6-e77ceff1a5f9.md`

## M49：三国杀 P4.4.2 批次 J2（界限突破 452–458/462：8 将 / 20 技能）✅

- **日期**：2026-10-31
- **完成**：界孙策（激昂/魂姿/制霸）、界夏侯渊（神速/设变）、界卧龙诸葛亮（八阵/藏拙/火计/看破）、界庞统（连环/涅槃）、界魏延（狂骨/奇谋）、界小乔（红颜/天香/飘零）、界孙坚（英魂/武烈）、界庞德（马术/鞬出）
- **机制**：`resolve_jiexian3.rs`（`Frame::JiangJie/HunziEnd/ZhiBaJie/SheBian/PiaoLingPlace/WuLie/NiePanSkill/LianHuanPick`）；`trick_used`（藏拙）、`jiandu_sha_bonus`、`mark_counts["lie"]`（武烈）、装备区转化（界火计/看破/连环/天香）、界看破无懈封锁
- **用例**：`tests/p97.rs` 25 条；`cargo test -p sanguosha` **736** 全绿；`p67` 池断言 35 → 43
- **已知边界**：见 `rules-rulings.md` §31
- **详见**：`memory_palace/plan/01a192cd-9c3e-7668-91f6-e77ceff1a5f9.md`

## M48：三国杀 P4.4.2 批次 J1（界限突破 26/304/446–451：8 将 / 16 技能 + 2 授予技）✅

- **日期**：2026-10-31
- **完成**：界黄忠（烈弓）、界徐庶（诛害/潜心 + ☆荐言）、界华雄（耀武/势斩）、界左慈（化身/新生）、界张角（雷击/鬼道/黄天）、界于吉（蛊惑 + 界缠怨）、界袁绍（乱击/血裔）、界刘禅（享乐/放权/若愚 + ☆思蜀）
- **机制**：`resolve_jiexian2.rs`（`Frame::ZhuHai/JianYan/ShiZhan/SiShu/XueYi/LeijiJie/LuanJiPick`）；界烈弓点数距离 + 双效果；`dealt:<seat>` 回合内伤害记录；界化身 3 张 + 换牌 + 势力/性别覆盖；界雷击判定后选目标（含使用【闪电】触发）；`mark_counts["yi"]`（裔）；`sishu_reverse` 判定反转；`start_virtual_trick_targets`（指定目标集合）
- **用例**：`tests/p96.rs` 26 条；`cargo test -p sanguosha` **711** 全绿；`p67` 池断言 27 → 35
- **已知边界**：见 `rules-rulings.md` §30（诛害按项目数据视为使用【杀】；荐言限一次；界蛊惑「弃牌或失体力」自动口径；界化身换牌固定移去非展示牌；界乱击最多少选 1 个目标）
- **详见**：`memory_palace/plan/01a192cd-9c3e-7668-91f6-e77ceff1a5f9.md`

## M47：三国杀 harness 三期（界限突破批次表 / 双包脚手架 / 用例稳定性指南）✅

- **日期**：2026-10-31
- **完成**：
  - `development-plan.md` 新增**界限突破剩余 60 位 J1–J8 批次表**（J8 为 4 将）。
  - `sgs-data-lib.mjs` 批次解析支持 `Y*`/`J*`；`sgs-batch-status` 同时输出两包批次进度与「下一批（界限突破）」；`sgs-batch-scaffold --pack jiexian` 取 J 批并提示「同名技能复用 code + `is_jiexian` 分支」。
  - `tests/support.rs` 新增 `stage_play` / `give_def_suit` / `equip_slot_card`（p95 已复用，删除本地重复）。
  - `check-sgs-skill-docs.mjs` 允许「主公/限定」合并标注（曹叡·兴衰）。
  - `batch-playbook.md` 增加界限突破批次说明与**「用例要抗随机」**（历史文本/相对变化/指定花色/座位不假定）四条准则。
- **验证**：`npm run sgs:check` 10/10；`cargo test -p sanguosha` 685；`sgs:batch-status` 显示 Y1–Y7 全 ✅、J1–J8 待开工（下一批 J1 26 界黄忠…451 界刘禅）。
- **详见**：`memory_palace/plan/01a19274-1de6-7668-91f6-e77bbdcc2f44.md`

## M46：三国杀 P4.4.2 批次 Y7（一将成名 388–395：8 将 / 17 技能，一将成名 72/72 收尾）✅

- **日期**：2026-10-31
- **完成**：辛宪英（忠鉴/才识）、吴苋（福绵/怠宴）、徐氏（问卦/伏诛）、曹节（守玺/惠民）、嵇康（清弦/绝响）、秦宓（谏征/专对/天辩）、薛综（复难/诫训）、蔡邕（辟撰/通博）
- **机制**：`Frame::ZhongJian/CaiShi/FuMian/DaiYan/WenGua/FuZhu/ShouXi/HuiMin/QingXian/JueXiang/JianZheng/ZhuanDui/FuNan/JieXun/TongBo`（`resolve_yijiang9.rs`）；天辩接入通用拼点（牌堆顶 + 红桃=K）、`draw_one_bottom_public`、守玺声明牌名限次、辟撰「书」标记
- **验证**：`cargo test -p sanguosha` **685** 全绿；`tests/p95.rs` 24 条（每将 ≥3）；`p67` 一将成名已实现数 **72（全部）**
- **已知边界**：伏诛不开放响应；福绵锦囊多目标未实现；清弦回复半未实现；绝响以清弦代替变体技能；诫训修改复难未实现；通博每次仅换 1 张（§29）
- **详见**：`skills.md` §2.7.8、`rules-rulings.md` §29、`development-plan.md` P4.4.2 批次 Y7

## M45：三国杀 P4.4.2 批次 Y6（一将成名 380–387：8 将 / 14 技能）✅

- **日期**：2026-10-31
- **完成**：郭皇后（矫诏/殚心）、李严（督粮/腹鳞）、孙登（匡弼）、刘虞（止戈/宗祚）、岑昏（极奢/链祸）、孙资刘放（瑰藻/讥谀）、黄皓（寝情/贿生）、张让（滔乱）
- **机制**：`Frame::JiaoZhao/DanXin/DuLiang/KuangBi/ZhiGe/JiSheEnd/GuiZao/JiYu/QinQing/HuiSheng/TaoLuan`（`resolve_yijiang8.rs`）；`jiazhao:*` 转化标记、`jiyu_banned` 花色封锁、`taoluan_used:*` 牌名限次、开局宗祚与死亡减上限
- **验证**：`cargo test -p sanguosha` **661** 全绿；`tests/p94.rs` 24 条（每将 ≥3）；`p67` 一将成名已实现数 64
- **已知边界**：矫诏「闪」仅响应未实现；腹鳞仅统计督粮；滔乱锦囊限 6 种（§28）
- **详见**：`skills.md` §2.7.7、`rules-rulings.md` §28、`development-plan.md` P4.4.2 批次 Y6

## M44：三国杀 P4.4.2 批次 Y5（一将成名 335–342：8 将 / 14 技能）✅

- **日期**：2026-10-31
- **完成**：刘谌（战绝/勤王）、夏侯氏（樵拾/燕语）、张嶷（怃戎/矢志）、孙休（宴诛/兴学/诏缚）、朱治（安国）、全琮（邀名）、公孙渊（怀异）、郭图逢纪（急攻/饰非）
- **机制**：`Frame::QiaoShi/YanYu/WuRong/YanZhu/XingXue/ZhaoFu/AnGuo/YaoMing/HuaiYi/JiGong`（`resolve_yijiang7.rs`）；战绝虚拟【决斗】+ 受伤摸牌、诏缚攻击范围覆盖、饰非复用响应窗口（视为出【闪】）
- **验证**：`cargo test -p sanguosha` **637** 全绿；`tests/p93.rs` 24 条（每将 ≥3）；`p67` 一将成名已实现数 56
- **已知边界**：战绝按本回合【决斗】伤害近似；兴学/安国选牌随机；诏缚只影响攻击范围（§27）
- **详见**：`skills.md` §2.7.6、`rules-rulings.md` §27、`development-plan.md` P4.4.2 批次 Y5

## M43：三国杀 P4.4.2 批次 Y4（一将成名 327–334：8 将 / 17 技能）✅

- **日期**：2026-10-31
- **完成**：孙鲁班（谮毁/骄矜）、朱桓（奋励/平寇）、顾雍（慎行/秉壹）、沮授（渐营/矢北）、蔡夫人（窃听/献州）、曹叡（恢拓/明鉴/兴衰）、曹休（千驹/倾袭）、钟繇（活墨/佐定）
- **机制**：`Frame::ZanHui/JiaoJin/FenLi/PingKou/ShenXing/BingYi/QieTing/XianZhou/HuiTuo/MingJian/QingXi/ZuoDing`（`resolve_yijiang6.rs`）；`targeted_others`/`phase_damage`/`fenli_skipped`/`huomo_used` 计数、`mingjian` 标记、QuickJudge `huituo` 判定分支
- **验证**：`cargo test -p sanguosha` **613** 全绿；`tests/p92.rs` 24 条（每将 ≥3）；`p67` 一将成名已实现数 48
- **已知边界**：谮毁仅【杀】；活墨不含【闪】；佐定仅【杀】目标窗口；兴衰自动结算；秉壹/献州自动选区（详见 §26）
- **详见**：`skills.md` §2.7.5、`rules-rulings.md` §26、`development-plan.md` P4.4.2 批次 Y4

## M42：三国杀 P4.4.2 批次 Y3（一将成名 187/188/321–326：8 将）✅

- **日期**：2026-10-31
- **完成**：王异（贞烈/秘计）、关兴张苞（父魂）、曹真（司敌）、韩浩史涣（慎断/勇略）、陈群（品第/法恩）、吴懿（奔袭）、周仓（忠勇）、张松（强识/献图）
- **机制**：`Frame::ZhenLie/MiJi/SiDi/ShanDuan/YongLue/PinDi/BenXi/ZhongYong/QiangShi/XianTu`（`resolve_yijiang5.rs`）；贞烈取消目标、司敌颜色封锁、奔袭距离累计+四选二强化、品第类型位图、献图阶段末惩罚
- **验证**：`cargo test -p sanguosha` **589** 全绿；`tests/p91.rs` 24 条（每将 ≥3）；`p67` 一将成名已实现数 40
- **已知边界**：贞烈仅普通锦囊；父魂虚拟杀不触发按牌名加成；奔袭锦囊强化未实现；强识只统计主动使用（§25）
- **详见**：`skills.md` §2.7.4、`rules-rulings.md` §25、`development-plan.md` P4.4.2 批次 Y3

## M41：三国杀批次 harness 二期（脚手架 + 共享数据 lib + 用例边界修复）✅

- **日期**：2026-10-31
- **完成**：
  - `apps/scripts/sgs-data-lib.mjs`：scripts 共用数据读取（ready/data.rs/批次表/p67），`sgs-batch-status.mjs` 改为复用（行为不变）。
  - `apps/scripts/sgs-batch-scaffold.mjs` + `npm run sgs:batch-scaffold`：自动取下一批（或 `--batch/--gids/--module/--tests`）打印武将元数据、`skills.rs` 常量+match 模板、`skills.md` 表行、用例/文档/收尾清单。
  - `tests/support.rs` 新增 `damage_and_advance`（伤害后技能用例高频样板），`p90` 已复用。
  - 修复 `check-sgs-skill-cases.mjs` 用例体边界（按顶层 `fn` 截断）：此前测试之间的私有助手会被算进上一条用例覆盖，Y2 出现误判；`p89` 助手已移到文件顶部，关平/朱然用例补显式技能 code 断言。
  - `batch-playbook.md` 增补「开工脚手架」与「覆盖统计口径」两条坑位。
- **验证**：`npm run sgs:check` 10/10；`cargo test -p sanguosha` 565；`sgs:batch-scaffold` 输出 Y3 8 将清单正确。
- **详见**：`memory_palace/plan/01a19274-1de6-7668-91f6-e77bbdcc2f44.md`「harness 优化项」

## M40：三国杀 P4.4.2 批次 Y2（一将成名 179–186：8 将）✅

- **日期**：2026-10-31
- **完成**：刘封（陷嗣）、潘璋马忠（夺刀/暗箭）、虞翻（纵玄/直言）、朱然（胆守）、伏皇后（惴恐/求援）、李儒（灭计/绝策/焚城）、法正（恩怨/眩惑）、徐庶（无言/举荐）
- **机制**：`Frame::XianSi/DuoDao/ZongXuan/ZhiYan/DanShou/ZhuiKong/QiuYuan/MieJi/JueCe/FenCheng/EnYuan/XuanHuo/JuJian`（`resolve_yijiang4.rs`）；「逆」标记、`zhuikong:*` 距离覆盖、`danshou_used`/`xiansi_used`/`fencheng_used`、虚拟【杀】复用、`deny:*` 目标封锁
- **验证**：`cargo test -p sanguosha` **565** 全绿；`tests/p90.rs` 24 条（每将 ≥3）；`p67` 一将成名已实现数 32；`npm run sgs:check` 10/10、`sgs:check:full` 11/11
- **harness**：修复 `check-sgs-skill-cases.mjs` 用例体边界（按顶层 `fn` 切分，避免测试之间的助手被算进上一条用例覆盖）
- **已知边界**：恩怨获牌半仅覆盖眩惑；纵玄/直言/灭计/焚城选牌按自动口径（详见 §24）
- **详见**：`skills.md` §2.7.3、`rules-rulings.md` §24、`development-plan.md` P4.4.2 批次 Y2

## M39：三国杀批次 harness（测试助手 + 聚合门禁 + 进度脚本 + playbook）✅

- **日期**：2026-10-31
- **完成**：
  - `crates/engines/sanguosha/src/tests/support.rs`：公共测试助手（`give_card/give_def/give_suit/give_rank/give_equip/set_top/find_cards/run_phase/pending_skill/pending_options_with`），`p89` 已改为复用，后续批次不再复制样板。
  - `apps/scripts/check-sgs-all.mjs` + `npm run sgs:check`（`sgs:check:fast` / `sgs:check:full`）：一条命令跑完 Rust 单测 + 4 个文档/数据/批次门禁 + pack-pool/view-contract/interaction + Web/e2e（`--full` 追加 MCP 冒烟），逐步计时 + 失败汇总。
  - `apps/scripts/sgs-batch-status.mjs` + `npm run sgs:batch-status`：按包统计已实现/剩余，解析 development-plan 批次表给出完成度与下一批名单，校验 `p67` 一将成名计数（`--check` 不一致 exit 1，已接入聚合门禁）。
  - `docs/games/sanguosha/batch-playbook.md`：批次 7 步流程、技能/帧/用例/文档模板、门禁命令、常见坑、DoD；`AGENTS.md` 索引与 `development-plan.md` 已引用。
- **验证**：`npm run sgs:check:fast` 8/8 通过；`cargo test -p sanguosha` 541 全绿；`sgs:batch-status` 显示一将成名 24/72、Y1 ✅、下一批 Y2 名单正确。
- **详见**：`memory_palace/plan/01a19274-1de6-7668-91f6-e77bbdcc2f44.md`「harness 优化项」

## M38：三国杀 P4.4.2 批次 Y1（一将成名 171–178：8 将）✅

- **日期**：2026-10-31
- **完成**：将华雄（恃勇）、刘表（自守/宗室）、钟会（权计/自立 + 授予排异）、曹冲（称象/仁心）、郭淮（精策）、满宠（峻刑/御策）、关平（龙吟）、简雍（巧说/纵适）
- **机制**：`Frame::ChengXiang/RenXin/ZiLi/PaiYi/ZiShou/JunXing/YuCe/LongYin/QiaoShuo`（`resolve_yijiang3.rs`）；「权」武将牌标记 + 手牌上限、`jiu_boost` 酒杀标记、自守伤害防止、精策花色/类型位图、龙吟不计杀次数、巧说拼点（赢→多目标 / 没赢→禁锦囊）、纵适拼点牌回收
- **验证**：`cargo test -p sanguosha` **541** 全绿；`tests/p89.rs` 24 条（每将 ≥3）；`p67` 一将成名已实现数 24
- **已知边界**：权计置「权」自动随机；巧说只实现「多指定一个目标」；精策按 `play:` 记录（转化牌不计）；称象亮出牌暂存处理区
- **详见**：`skills.md` §2.7.2、`rules-rulings.md` §23、`development-plan.md` P4.4.2 批次 Y1

## M37：三国杀视图增强 —— 手牌上限展示 ✅

- **日期**：2026-10-30
- **背景**：部分技能会修改弃牌阶段手牌上限（英姿=体力上限、将驰/奇才/清俭等 +N、克己=无上限），不能由体力推断。
- **完成**：引擎公共视图/个人区新增 `hand_limit`（`null` = 无上限，如克己）；Web 公共座位、个人区标题、详情浮层、文本态统一显示 `手牌数/上限`（∞ 表无上限）。
- **验证**：`cargo test -p sanguosha` **517** 全绿（`tests/p88.rs` 2 条：英姿上限=体力上限、克己 null）；`check:sgs-view-contract` 字段集同步；`test:web` / `test:sgs-web` / MCP 冒烟。
- **详见**：`ui-spec.md` §3.2、`view.rs`、`apps/web/src/view/sgs/{public,personal,detail,text}.ts`

## M36：三国杀 P4.4.2 授予技能补齐（清侧 / 怀柔）✅

- **日期**：2026-10-30
- **完成**：`EXTRA_SKILLS` 的 `qingce`（毌丘俭·鸿举）与 `huairou`（陆抗·破势）改为 `implemented:true`
- **机制**：清侧（`Frame::QingCe`：移去「荣」+ 弃其他角色装备/延时锦囊，不能弃自己）；怀柔（重铸装备牌：弃置并摸 1）
- **验证**：`cargo test -p sanguosha` **515** 全绿；`tests/p87.rs` 4 条；`check:sgs-skill-cases` 不再有授予技能告警
- **详见**：`skills.md` §4.1、`rules-rulings.md` §22、`development-plan.md` P4.4.2 授予技能补齐、ADR-0029

## M35：三国杀 P4.4.2 批次 6（一将成名批次 2：8 将）✅

- **日期**：2026-10-30
- **机制**：绝情、伤逝、将驰、奇策/智愚、当先/伏枥、潜袭、安恤/追忆、疠火/醇醪、弓骑/解烦（新增 `resolve_yijiang2.rs`，9 个帧）
- **技能**：张春华（绝情/伤逝）、曹彰（将驰）、荀攸（奇策/智愚）、廖化（当先/伏枥）、马岱（马术/潜袭）、步练师（安恤/追忆）、程普（疠火/醇醪）、韩当（弓骑/解烦）
- **验证**：`cargo test -p sanguosha` **511** 全绿；`tests/p86.rs` 24 条；一将成名已实现 16 位
- **详见**：`skills.md` §2.7.1、`rules-rulings.md` §21、`development-plan.md` P4.4.2 批次 6

## M34：三国杀 P4.4.2 批次 5（界标·群 4 将，界标 25 将完成）✅

- **日期**：2026-10-30
- **机制**：除疠（势力去重弃牌）、利驭（拿牌/装备触发决斗，`pending_liyu` 栈空结算）、义从（双向距离）、趫猛（弃牌/得坐骑）、界离间可弃装备且无无懈、界闭月空手摸 2、界急救同标准
- **技能**：界华佗（急救/除疠）、界吕布（无双/利驭）、界公孙瓒（义从/趫猛）、界貂蝉（离间/闭月）
- **验证**：`cargo test -p sanguosha` **487** 全绿；`tests/p85.rs` 12 条；界标 25 将全部可用（池 27 位 = 标准 25 全替换 + 李典/公孙瓒）
- **详见**：`docs/games/sanguosha/skills.md` §2.8.4、`rules-rulings.md` §20、`development-plan.md` P4.4.2、ADR-0024

## M33：三国杀 P4.4.2 批次 4（界标·魏 8 将）✅

- **日期**：2026-10-30
- **机制**：界奸雄摸 1+获得、界护驾主公摸 1、鬼才装备改判、界刚烈红伤/黑弃、清俭、界突袭少摸等量、界裸衣亮 3 + 跨回合伤害 +1、恂恂、忘隙、界洛神不计手牌上限
- **技能**：界曹操（奸雄/护驾）、界司马懿（反馈/鬼才）、界夏侯惇（刚烈/清俭）、界张辽（突袭）、界许褚（裸衣）、界郭嘉（天妒/遗计）、界李典（恂恂/忘隙）、界甄姬（倾国/洛神）
- **验证**：`cargo test -p sanguosha` **475** 全绿；`tests/p84.rs` 24 条
- **详见**：`docs/games/sanguosha/skills.md` §2.8.3、`rules-rulings.md` §19、`development-plan.md` P4.4.2、ADR-0024

## M32：三国杀 P4.4.2 批次 3（界标·吴 8 将）✅

- **日期**：2026-10-30
- **机制**：奋威多目标无效、勤学觉醒（授予攻心）、博图额外回合（四花色掩码）、界苦肉弃牌、诈降（失体力摸 3 / 红杀不可抵消）、界英姿手牌上限=体力上限、界反间、界国色限一次+选项二、界谦逊移出手牌、界连营延迟询问、界制衡弃光额外摸 1、界救援转移回复、界结姻体力比较
- **技能**：界甘宁（奇袭/奋威）、界吕蒙（克己/勤学/博图）、界黄盖（苦肉/诈降）、界周瑜（英姿/反间）、界大乔（流离/国色）、界陆逊（谦逊/连营）、界孙权（制衡/救援）、界孙尚香（枭姬/结姻）
- **验证**：`cargo test -p sanguosha` **451** 全绿；`tests/p83.rs` 24 条
- **详见**：`docs/games/sanguosha/skills.md` §2.8.2、`rules-rulings.md` §18、`development-plan.md` P4.4.2、ADR-0024

## M31：三国杀 P4.4.2 批次 2（界标·蜀 7 将）✅

- **日期**：2026-10-30
- **机制**：义绝（非锁定技失效+封手牌）、界铁骑（弃同花色牌否则不能闪）、界涯角（展示牌堆顶二选一）、界仁德（每目标限一次+第二张基本牌）、界观星（3/5 张 + 全置底结束阶段再发动）、界集智（弃基本牌换手牌上限）、界奇才（防具/宝物防弃）、界武圣方片杀无距离、界龙胆酒/桃互转、界激将代出杀主公摸 1、界咆哮追击 +1、替身限定回复+摸牌
- **技能**：界刘备（仁德/激将）、界关羽（义绝/武圣）、界张飞（替身/咆哮）、界赵云（涯角/龙胆）、界马超（马术/铁骑）、界诸葛亮（空城/观星）、界黄月英（集智/奇才）；新增 `resolve_jiexian.rs`（4 个帧）
- **验证**：`cargo test -p sanguosha` **427** 全绿；`tests/p82.rs` 21 条；界优先去重（p67）改为「蜀 7 替换原版」
- **已知边界**：`skills.md` §2.8.1（义绝仅按 `has` 屏蔽；观星全置底以右列为判据；界激将只覆盖代出杀；涯角不覆盖濒死桃）
- **详见**：`docs/games/sanguosha/skills.md` §2.8/§2.8.1、`development-plan.md` P4.4.2、ADR-0024

## M30：三国杀 P4.4.2 批次 1（ADR-0024 + 数据扩包 + 一将成名 8 将）✅

- **日期**：2026-10-30
- **口径**（ADR-0024）：界化武将 code 独立 / 技能 code 复用 / 选将池「界优先去重」（`lib.rs::prefer_jiexian`）/ 旧局不变
- **数据**：`gen-sanguosha-engine-data` 登记 `yijiang`(72) + `jiexian`(87)，`data.rs` 全量 **232** 位；`GENERAL_PACKS` 新增两包（未实现不进池）
- **机制**：弃牌堆来源钩子（落英/旋风）、移出游戏并回合结束回置（破军 `void_return`）、陷阵无视防具/追加目标/杀不计上限、伤害防止获牌（制蛮）、濒死展示手牌（补益）、装备区交换（甘露）
- **技能**：曹植（落英/酒诗）、高顺（陷阵/禁酒）、陈宫（明策/智迟）、凌统（旋风）、马谡（散谣/制蛮）、吴国太（甘露/补益）、徐盛（破军）、于禁（镇军）
- **验证**：`cargo test -p sanguosha` **396** 全绿；`check:sgs-engine-data`（232 将）、`check:sgs-skill-cases`（81 位）、`check:sgs-skill-docs`（§2.7）、`check:sgs-view-contract`、`npm run test:web`、`npm run test:sgs-web`、MCP 冒烟
- **已知边界**：ADR-0024 §3（界标 25 将待实现、去重按「界」前缀名匹配）+ §16（禁酒响应不提供、【杀】不计上限口径）
- **详见**：`docs/games/sanguosha/skills.md` §2.7/§2.8、`development-plan.md` P4.4.2、ADR-0024、`rules-rulings.md` §16

## M29：三国杀 P4.4.1 SP-2 8 将（风林火山 48 将完成）✅

- **日期**：2026-10-30
- **机制**（ADR-0029）：武将牌标记「任/荣/箜声」；废除装备栏/判定区；延迟结算队列（镇骨）；牌进出游戏钩子（良姻）；不可响应 + 本阶段/本回合手牌封禁（往烈/雄乱）；回合统计（罪论条件）；特殊区 `zuilun`
- **技能**：卢植·明任/贞良（转换）；郝昭·镇骨；陈到·往烈；诸葛瞻·罪论/父荫；周妃·良姻/箜声；毌丘俭·征荣/鸿举；陆抗·谦节/决堰/破势；张绣·雄乱/从谏
- **验证**：`cargo test -p sanguosha` **369** 全绿；`check:sgs-skill-cases`（352 条 / 73 位已实现武将 ≥3 + 转换技两面）、`check:sgs-skill-docs`（137 条一致）、`check:sgs-engine-data`、`check:sgs-view-contract`、`npm run test:web`、`npm run test:sgs-web`、MCP 冒烟
- **已知边界**：ADR-0029 §3（清侧/怀柔 `implemented:false`；良姻仅 void 区；往烈限【杀】/单目标锦囊；雄乱封手牌不封装备与技能；镇骨至多 5）
- **详见**：`docs/games/sanguosha/development-plan.md` P4.4.1 SP-2 记录、`skills.md` §2.6、ADR-0029、`rules-rulings.md` §15

## M28：三国杀 P4.4.1 SP-1 8 将 ✅

- **日期**：2026-10-30
- **机制**（ADR-0028）：转换技（`flags["yj:*"]`）；计数标记（`mark_counts` + 视图 `marks_count`）；牌堆底摸牌（寸目）/置顶（恃才）；伪帝借用主公技；回合内禁止指定目标（`turn_counters["deny:*"]`）；出牌阶段伤害防止与用牌计数（掣政）；审时（阴）私有观看手牌 + 回合结束校验
- **技能**：袁术·庸肆/伪帝；王基·奇制/进趋；严颜·拒战（转换）；王平·飞军/兵略；陆绩·怀橘/遗礼/整论；孙亮·溃诛/掣政/立军；蒯越蒯良·荐降/审时（转换）；许攸·成略（转换）/恃才/寸目
- **验证**：`cargo test -p sanguosha` **344** 全绿；`check:sgs-skill-cases`（326 条 / 65 位已实现武将 ≥3 + 转换技阳阴两面）、`check:sgs-skill-docs`（122 条技能登记一致）、`check:sgs-engine-data`（160 牌 / 73 将）、`check:sgs-view-contract`、`npm run test:web`、`npm run test:sgs-web`、MCP 冒烟
- **harness**：转换技两面覆盖门禁；文档-代码一致性门禁 `check:sgs-skill-docs`；Web 视图契约快照（`marks_count`/阳阴/旧快照）
- **已知边界**：ADR-0028 §3（奇制每次使用一次；飞军只弃手牌；兵略按目标首次；审时阴只写日志；恃才装备不触发；掣政弃牌随机；立军收益自动）
- **详见**：`docs/games/sanguosha/development-plan.md` P4.4.1 SP-1 记录、`skills.md` §2.5、ADR-0028、`rules-rulings.md` §14

## M27：三国杀 P4.4.1 山包 8 将 ✅

- **日期**：2026-10-30
- **机制**（ADR-0027）：觉醒/额外技能（`extra_skills` + `grant_skill`）；屯田标记/距离；巧变阶段跳过；享乐反制；放权额外回合（`extra_turn`）；制霸拼点夺牌；固政弃牌回收（`phase_discarded`）；化身/新生（`huashen`）；悲歌/断肠（`skills_lost`）
- **技能**：张郃·巧变；邓艾·屯田/凿险；姜维·挑衅/志继；刘禅·享乐/放权/若愚；孙策·激昂/魂姿/制霸；张昭张纮·直谏/固政；左慈·化身/新生；蔡文姬·悲歌/断肠
- **验证**：`cargo test -p sanguosha` **317** 全绿；`check:sgs-skill-cases`（299 条 / 57 位已实现武将 ≥3）、`check:sgs-engine-data`（73 将）、`check:sgs-view-contract`、`check:interaction-primitives`、`npm --prefix apps run test:web`、MCP 冒烟通过
- **已知边界**：ADR-0027 §3（放权自动弃第一张；巧变移动复用解围；化身只取第一个已实现技能；固政自动分配；悲歌致命伤害不触发；制霸仅魂姿后可拒绝）
- **详见**：`docs/games/sanguosha/development-plan.md` P4.4.1 山包记录、`skills.md` §2.4、ADR-0027、`rules-rulings.md` §13

## M26：三国杀 P4.4.1 林包 8 将 ✅

- **日期**：2026-10-30
- **机制**（ADR-0026）：行殇死亡接管（死亡清理前询问，终局不触发）；南蛮免疫/祸首伤害来源/巨象获得；再起/好施摸牌替代；崩坏体力上限减少；帷幕黑色锦囊目标过滤；放逐翻面+摸牌；烈刃拼点父帧 outcome 保护；乱武限定多角色顺序；缔盟交换手牌；完杀/颂威/暴虐/酒池/肉林/截辎
- **技能**：徐晃·断粮/截辎；曹丕·行殇/放逐/颂威；孙坚·英魂；董卓·酒池/肉林/崩坏/暴虐；祝融·巨象/烈刃；孟获·祸首/再起；贾诩·完杀/乱武/帷幕；鲁肃·好施/缔盟
- **验证**：`cargo test -p sanguosha` **293** 全绿；`check:sgs-skill-cases`（275 条 / 49 位已实现武将 ≥3）、`check:sgs-engine-data`（73 将）、`check:sgs-view-contract`、`check:interaction-primitives`、`npm --prefix apps run test:web`、MCP 冒烟通过
- **已知边界**：ADR-0026 §3（行殇终局不触发、乱武自动取第一张杀/最近目标且不触发目标指定后窗口、好施先选牌再选目标、帷幕只过滤目标枚举）
- **详见**：`docs/games/sanguosha/development-plan.md` P4.4.1 林包记录、`skills.md` §2.3、ADR-0026、`rules-rulings.md` §12

## M25：三国杀 P4.4.1 火包 8 将 ✅

- **日期**：2026-10-30
- **机制**（ADR-0025）：通用拼点 `PindianFrame`（双方选牌比点、同点发起者未赢、`Outcome::Pindian`）；`virtual_equip` 八阵（防具槽空时视为八卦阵）；天义无距离/多目标（`ShaFrame.no_distance/tianyi`）；涅槃限定自救（弃置所有区域 + 复原武将牌 + 摸 3 + 回复至 3）；鞬出目标指定后弃牌分支；双雄判定获得 + 异色当决斗；血裔主公技手牌上限
- **技能**：典韦·强袭；荀彧·驱虎/节命；庞统·连环/涅槃；卧龙诸葛亮·八阵/火计/看破；太史慈·天义；庞德·马术/鞬出；颜良文丑·双雄；袁绍·乱击/血裔
- **验证**：`cargo test -p sanguosha` **267** 全绿；`check:sgs-skill-cases`（249 条 / 41 位已实现武将 ≥3）、`check:sgs-engine-data`（73 将）、`check:sgs-view-contract`、`check:interaction-primitives`、`npm --prefix apps run test:web`、MCP 冒烟通过
- **已知边界**：ADR-0025 §3（天义与方天不叠加、流离优先于鞬出、涅槃不重置铁索/限定/觉醒、八阵不产生牌实例）
- **详见**：`docs/games/sanguosha/development-plan.md` P4.4.1 火包记录、`skills.md` §2.2、ADR-0025、`rules-rulings.md` §11

## M24：三国杀 P4.4.0 / P4.4.1 —— 风林火山包开关 + 风包 8 将 ✅

- **日期**：2026-10-30
- **P4.4.0 数据/开关**：生成脚本登记 `wind_fire_forest_mountain`（48 位，官网标签全量），默认生成标准 25 + 风林火山 48 = **73 将**；`Config.general_packs`（默认 `["standard"]`，旧快照 serde 兼容）+ `parse_general_packs` + lobby `lobby:set_general_packs:standard|standard+wind_fire_forest_mountain`；选将池按包过滤 + `skills::general_ready`（未实现武将不进池）
- **P4.4.1 风包 8 将**：魏延（狂骨/奇谋）、夏侯渊（神速）、曹仁（据守/解围）、小乔（红颜/天香）、黄忠（烈弓）、周泰（不屈/奋激）、于吉（蛊惑 + 缠怨）、张角（鬼道/黄天/雷击）
- **机制**（ADR-0023）：`Seat.marks`（不屈「创」，牌数守恒覆盖）、`Seat.flipped`（翻面跳过/翻回）、天香伤害防止窗口、蛊惑质疑/真假/缠怨、神速虚拟无距离【杀】、鬼道改判、`Frame::*` 8 类新帧
- **视图/Web**：公共/个人区下发 `marks`/`flipped`，`view_contract` 期望集同步；Web 座位/个人区展示翻面与标记
- **验证**：`cargo test -p sanguosha` **242** 全绿；`check:sgs-engine-data`（160 牌 / 73 将）、`check:sgs-skill-cases`（224 条 / 33 位已实现武将 ≥3）、`check:sgs-view-contract`、`check:interaction-primitives`、`npm --prefix apps run test:web`、`npm run verify:sanguosha`（含包开关）全通过
- **已知边界**：ADR-0023 §3（鬼道/鬼才仅手牌、解围移动限自己区域、奋激触发点有限、神速②仅手牌装备、不屈无牌不发动）
- **详见**：`docs/games/sanguosha/development-plan.md` P4.4 记录、`skills.md` §2.1、`data-model.md` §3.7/§4.1、ADR-0023

## M23：三国杀 P4.2 / P4.3 / P4.5 —— 装备效果 + 军争结算 + 装备技能化 ✅

- **日期**：2026-10-30
- **P4.2**：标准 8 件装备效果全部实现（寒冰剑 damage_before、雌雄双股剑、青龙偃月刀/贯石斧 after_dodge、丈八蛇矛主动+响应、方天画戟多目标、八卦/麒麟等既有）
- **P4.3**：军争 52 张全部接入结算——酒、火杀/雷杀（`ShaFrame.damage_type`）、火攻、铁索连环（横置/重铸/属性传导）、兵粮寸断、藤甲、白银狮子、古锭刀、朱雀羽扇；`PENDING_JUNZHENG_CARDS` 清空
- **P4.5**：ADR-0022 装备技能化落地——`EQUIPMENT_SKILLS`（21 件）+ `Trigger` 六类 + `triggered()`；AfterDodge/DamageBefore/TargetChosen/PlayPhase handler 表；`player_area.equipment_skills[]`；Web 武器高亮（`option_sources`）
- **验证**：`cargo test -p sanguosha` **210** 用例；`check:sgs-skill-cases`（193 条 / 25 将 ≥3）、`check:sgs-view-contract`、`check:interaction-primitives`、`fmt --check`、`npm --prefix apps run test:web`、MCP 冒烟全通过
- **详见**：`docs/games/sanguosha/development-plan.md` P4.2/P4.3/P4.5 记录、`skills.md`、ADR-0022

## M22：三国杀 P4.1 —— 标准包全技能（25 将 / 49 技能）✅

- **日期**：2026-10-30（P4.1 收口）
- **完成**：
  - **改判窗口**：`judge_rewrite` pending（判定帧 `Reveal→Rewrite→WaitRewrite→Effect`）+ 鬼才（司马懿）；判定牌亮出即处理区/公开
  - **锁定/目标过滤**：空城（诸葛亮）、谦逊（陆逊）、马术（马超）、咆哮（张飞）、无双（吕布）、倾国（甄姬）
  - **转化**：武圣、龙胆、急救、倾国、国色（`SgsState.virtual_defs` + `effective_def_code`）
  - **主动/伤害后**：苦肉、青囊、结姻、反馈、遗计（逐张分配）、刚烈、反间、洛神、铁骑、天妒、裸衣（手动 `choice`）、英姿、连营、枭姬、流离、闭月/离间/奇袭/突袭/仁德/激将/制衡/救援/奸雄/护驾/集智/奇才/克己/观星
  - **工程**：`RespondFrame.need/played/no_shan/duel_initiator`（无双/铁骑）、`Frame::Liuli/Luoshen/Ganglie/Fanjian/Qingnang/Jieyin/Yiji/Luoyi`
  - **harness**：`check:sgs-skill-cases`（扫描测试体，标准 25 将每人 ≥3 条用例）；D 回归 12 条跨技能交互（p47）
- **验证**：`cargo test -p sanguosha` **164** 用例全绿；`check:sgs-skill-cases` / `check:sgs-view-contract` / `check:interaction-primitives` / `fmt --check` / MCP 冒烟（160 张牌、守恒、终局）通过
- **已知偏差（已登记 `skills.md` §8）**：遗计按「每次伤害结算」触发（规则为每点）；铁骑/刚烈/洛神为自动判定（待 P4.2 通用 `judge_before`）；流离弃置限手牌；国色判定区视图仍显示物理牌
- **详见**：`docs/games/sanguosha/development-plan.md` P4.1 记录、`skills.md`（技能表/钩子/偏差）、`data-model.md` §4.1（`virtual_defs`）、`legal-actions.md`

## M21：三国杀 P4.0 —— 军争牌堆 + 包开关（数据层）✅

- **日期**：2026-10-30
- **完成**：
  - 生成脚本：军争 52 张入 `CARD_DEFS`/`JUNZHENG_DECK`，`PhysicalCard.pack` + `GeneralDef.pack`，`deck_cards(&packs)`；`check:sgs-engine-data` 扩围至 160 张（标准 108 + 军争 52）/ 25 将
  - 引擎：`Config.packs`（默认 `["standard","junzheng"]` = 160）、`parse_packs`/`packs_mode`、`lobby:set_packs:standard|standard+junzheng`、`finish_setup` 按 config 建堆、`deck_shuffled` 带 `packs`
  - 军争未实现结算的 10 种牌（酒/火杀/雷杀/火攻/铁索连环/兵粮寸断/古锭刀/朱雀羽扇/藤甲/白银狮子）在 `legal_actions` 与 `play:` 双重拦截；骅骝走通用距离逻辑
  - 测试：新增 `tests/p40.rs` 5 条；修复 160 牌堆下受影响的牌数/弃牌断言；`apps/verify-sanguosha.mjs` 守恒断言改为按 `config.packs` 计算
- **验证**：`cargo test -p sanguosha` **109** 用例全绿；`npm run check:sgs-engine-data` 通过；`cargo fmt --check -p sanguosha` 通过；MCP 冒烟（160 张/packs 打印 + 军争未实现牌零出牌断言）通过
- **harness（2026-10-30）**：新增「牌定义 ↔ 引擎结算覆盖」门禁（每张 CardDef ∈ 标准包 / 已实现军争牌 / `PENDING_JUNZHENG_CARDS`）+ `verify-sanguosha.mjs` 端到端断言军争未实现牌从未被列为可出
- **已知限制**：军争牌结算（属性伤害/连环/酒/兵粮/火攻/藤甲/白银狮子/古锭刀/朱雀羽扇）留 P4.2–P4.3；武将池仍为标准 25；中台 live 元数据 API 留 P5
- **详见**：`docs/games/sanguosha/development-plan.md`「P4.0 实施记录」、`data-model.md` §3.7/§4.1、`state-machine.md` §2.1、`legal-actions.md` §3/§4.2

## M20：三国杀 P3 —— Web 交互基座（三分区页面 + 特殊区 + 观星）✅

- **日期**：2026-10-30（P3.0–P3.6 收口；**真机验收 16 项由用户确认通过**）
- **完成**：
  - **数据与动作通路**：server 按 `game_id` 分派引擎投影（修掉「未注册引擎原样下发内部状态」泄漏）；`/internal/game/{gameId}/{roomId}/action` 单写者执行 + web `POST /tabletop_game/:gameId/action`（JSON / 无 JS 表单 303）
  - **页面**：三分区（公共只读 / 个人交互 / 特殊区浮层）、四档响应式布局、10s 轮询、无 JS 文本态（含 `arrange` 文字提交）
  - **交互**：`legal_actions` 驱动 + pending 逐条提交（目标/响应/弃牌/多选/接力）、居中浮层基座（blocking 语义）、指针手势（短按/长按/位移取消）
  - **特殊区**：无懈链、濒死、五谷、拿牌三组、借刀二阶段、技能多选；**观星 2×5**（点击/▲▼/长按拖拽吸附/重置/`arrange` 一次提交）
  - **本地/自动化**：`npm run sgs:web-room`（建局+签链接）、`npm run test:sgs-web`（playwright-core + 系统 Edge 四档视口/隐藏信息/越权/无 JS 表单/点击推进）、`npm run sgs:history`（完整历史）
- **验证**：`cargo test -p sanguosha` 103 用例；`npm run test:web` 五套；`npm run test:sgs-web` 全绿；真机 16 项通过（`p3-manual-test.md`）
- **已知限制**：真机吸附阈值/长按阈值按默认 10%/400ms（可调）；借刀二阶段无灰/红边框视觉；e2e 未覆盖无懈/五谷/观星整链（手工验证）；全仓 `cargo fmt` 历史漂移未处理
- **详见**：`docs/games/sanguosha/development-plan.md` P3 记录、`ui-spec.md`、`p3-manual-test.md`、`docs/modules/web.md`、计划台账

## M19：三国杀 P2.4 —— 技能内核 + 首批武将 + 交互原语 ✅

- **日期**：2026-10-28（P2.4 收口；记录补录 2026-10-30）
- **完成**：
  - 技能注册表（`skills.rs`，确定性派发：声明序 → 左手侧座位序 → 注册序）+ 技能帧（`resolve_skill.rs`）+ 第二批帧（`resolve_more.rs`）
  - 首批 8 将 14 技能（含主公技激将/护驾接力）+ 诸葛亮观星（P2.5）；装备可选效果：青釭剑/八卦阵/仁王盾/诸葛连弩（麒麟弓 P3 追加）
  - 交互原语：`choice` / `choose_many(min,max)` / `ordered`（观星）/ 跨玩家响应队列；视图 `player_area.skills` 结构化
- **验证**：`cargo test -p sanguosha` 77 用例（每将 ≥3 用例）；MCP 终局冒烟；workspace 全绿
- **详见**：`docs/games/sanguosha/skills.md`、`development-plan.md` P2.4 记录

## M18：三国杀 P2 —— 引擎内核 + 身份局全流程 + 特殊区骨架 ✅

- **日期**：2026-10-28（P2.1–P2.5 收口；**人工验收 2026-10-28 通过**，用户裁定进入 P3）
- **完成**：
  - **P2.1 骨架**：`crates/engines/sanguosha`（EnginePlugin/lobby 5–8 人/发身份/选将/108 张标准牌堆/state schema/三分区投影/数据生成脚本+CI）
  - **P2.2 回合**：阶段机 `prepare→judge→draw→play→discard→end`、判定区 LIFO（乐/闪电）、杀/闪/桃、装备换槽与距离、持有上限、弃牌阶段、濒死求桃与死亡
  - **P2.3 结算栈**：`resolution: Vec<Frame>` + `outcome`（schema v2）、无懈链（全员询问/重开/奇偶）、延时锦囊判定前无懈、单目标锦囊（顺手/过河/无中/决斗/借刀）、AOE（南蛮/万箭/桃园）、死亡奖惩与胜负
  - **P2.4 技能**：技能注册表 + 固定派发顺序 + 技能帧；首批 8 将 14 技能 + 装备可选效果（青釭/八卦/仁王/连弩）；每将 ≥3 用例
  - **P2.5 特殊区**：`resolve_zone.rs` —— `wugu`（亮牌=存活数 → **释放者先选** → 座位顺序，每选择者一次无懈，剩余进弃牌堆）+ `guanxing`（2×5 槽契约、准备阶段自动进入、`arrange:<csv>` 校验张数/去重/来源/处理区、`settle_slots()` 跳空压缩、倒序回写牌堆）
  - **响应询问口径（2026-10-28 用户裁定）**：无手牌且无替代选项的玩家**自动跳过**（不再假装犹豫）；有手牌者即使没有对应牌仍显式 `pass`，不代选；空手同势力盟友不提供主公技接力（`special-zones.md` §4.1）
- **验证**：`cargo test -p sanguosha` **98 用例**、`cargo test --workspace` 全绿、`cargo fmt --check -p sanguosha` 通过、生成数据 `--check` 通过（108 牌/25 将）；MCP 冒烟 `node apps/verify-sanguosha.mjs` 跑通并命中 `wuxie/play/skill` 与特殊区 `wugu_zone/wugu_pick/guanxing_arrange`；牌数守恒每 40 步校验（含处理区）
- **收尾门禁**：`tests/replay.rs`（同 seed+同动作 → 状态与事件序列一致）、`npm run check:sgs-view-contract`（三分区字段级冻结）、`npm run check:interaction-primitives`（原语/基数校验）；CI 增加 `cargo fmt --check -p sanguosha`（全仓 fmt 因历史 156 处漂移未纳入门禁）
- **已知限制/偏差**：观星按「自动进入」实现（用户已定不可取消，不询问是否发动）；五谷亮牌先于无懈窗口（便于决策，与 AOE 每目标独立无懈一致）；空城与其余标准将技能、7 件装备选修效果、兵粮寸断/军争留 P4；Web 页面（含观星拖拽）留 P3
- **详见**：`docs/games/sanguosha/development-plan.md`（P2 实施记录）、`special-zones.md` §5/§6、`ui-spec.md` §7/§9.5、`skills.md`、`plan/01a17d9c-599e-7668-91f6-e76ea315b83f.md`、`plan/01a182be-f2ab-7668-91f6-e771e525ee05.md`

## M17：道具中台 MVP（三国杀 P1）✅

- **日期**：2026-10-26（人工验收 2026-10-26，用户裁定 P1 收口）
- **完成**：
  - **存储**：`prop_packs` / `prop_assets` / `sgs_identities` / `sgs_identity_setup` / `sgs_card_defs` / `sgs_cards` / `sgs_generals` / `sgs_skills` / `sgs_general_skills` / `prop_status_log`（含 status/equip_slot/range_type 约束）
  - **server API**：`/internal/gameprops/{summary,seed,import/heroes,status,{table}}`；静态 Admin token；单写者
  - **SPA**：`apps/props-admin`（Vue3 + Vite + Naive UI），含包/身份/配比/牌定义/物理牌/**定义↔物理牌关联页**/武将/技能/关联/导入
  - **数据**：`seed`（4 身份 / 20 配比 / 12 包）；`heroes.json` 导入 696 武将、1053 技能，幂等
  - **托管**：仅本机 Vite dev/preview（5174/4174）；**不挂公网**，P8 权鉴后再挂 web/server
  - **文档/CI**：`docs/modules/props-admin.md`；CI 增加 `test:web` + `build:props`
- **验证**：`cargo test --workspace` **136 通过**；`npm run test:web` 三套通过；`npm run build:props` 通过；手工 API：seed/import 幂等、状态流转；本机 Vite 静态/代理 200；公网入口未挂载
- **已知限制**：图片上传未实现；牌表人工录入（关联页逐张加花色/点数）；Admin token 为静态共享（完整权鉴 P8）；SPA 构建约 720KB
- **详见**：`docs/modules/props-admin.md`、`docs/games/sanguosha/data-model.md`、`plan/01a1736d-3571-7742-861e-3aa85e630def.md`

## M16：dev 线迁移 —— MCP 薄桥 + LangGraph 执行器 + n8n shadow（ADR-0013/0014/0015）✅

- **日期**：2026-10-20（人工验收 2026-10-20，用户裁定 done）
- **完成**（分批 B0–B9，按 `plan/01a15977` 执行）：
  - **B0 死代码清理**：删除 `crates/command-router`、`crates/push` + server 两条死依赖 + 过期模块文档；`AGENTS.md`/`README.md`/`architecture.md` 同步（ADR-0015 §8）
  - **B1 storage 底座**：`dev_task_events` / `dev_runs` / `dev_artifacts` 三表 + `dev_tasks` 加列 + 迁移守卫（事件幂等、并发 claim 单赢家均有单测）
  - **B2 server 内部 API**：`/internal/dev/*`（只读 payload / 事件追加 / 投影写 / 权限判定 / 事件查询 / runs / notify）
  - **B3 薄桥 + 原语 v2**：`apps/dev-bridge`（stdio MCP，19 工具）；gateway 只换连接目标；新增 **golden 契约快照**（`test-dev-contract.mts` + `fixtures/dev-contract.json`，改名/改描述即失败）
  - **B4 gateway 入口化**：移除 `runDev*`，只做「识别 → 落库 → 入队 → 回执」；审批拆两原语；事件 → passive 出站桥；新增 `#补充` 通道
  - **B5 n8n（shadow）**：`docker-compose` profile + `deploy/n8n/workflows/dev-shadow.json`（8 节点）；webhook 改 **HMAC-SHA256 + ±300s 时间窗 + 一次性签名（static data nonce）**
  - **B6/B6.2 LangGraph 执行器**：`agents/dev_executor`（shadow 只跑 plan）→ `planner`（`DEV_PLANNER=stub|llm|deepagents`）+ 计划 diff
  - **B7 状态权威切 LangGraph（L3）**：投影 + **单活跃 run** + 对账作业（实测 0 差异）+ 事件 → 出站通知（轮询 `list_events`，Redis 游标去重，`DEV_NOTIFY_ENABLED=0` 可关）
  - **B8 引擎删除后回归**：删除 `crates/dev-task`、`crates/dev-task-mcp`、`dev-session.ts`、`data/dev-sessions/`；`dev-task-client` 默认连桥（无 Rust 回退）
  - **B9 CI + 部署**：`.github/workflows/ci.yml` 三 job（Rust/Node/Executor）；本机部署器 `deploy-local.mjs`（status/history/deploy/rollback/serve）；n8n **两段式人工批准**（request → approve，实测 dry-run）；`notify_user` 原语（server `/internal/dev/notify` → `notify_requested` → 网关出站）
- **验证**：`cargo test --workspace` **132 全绿**；`npm run test:agent-gateway` **18/18**（后续新增叙述主体用例后 19/19）；`node apps/scripts/status.mjs` → ok；真机全生命周期：QQ 发起 → 回执 → 计划短链 → 确认 → 执行 → 验收；**gateway 重启不丢**、取消后不再推送、`#补充` 生效；n8n shadow 真机 + HMAC 加固验证；对账 0 差异；`check:proactive-gate` 通过
- **关键结论**：
  - 「原 Rust dev 引擎可废弃」成立，但 `dev_task_events` / `dev_runs` / 投影 / 租约是**切换前必补项**（ADR-0014 结论被 ADR-0015 继承）；n8n 承接 Git/CI/部署属 P3+，不与 P0/P1 捆绑
  - `sessionId` 仍是统一键（任务 / 计划文档 / 执行器 run / web 短链）
  - 分批原则（读取预算 ≤6 文件/≤2000 行、每批独立可回滚 commit、批末交接卡）实测有效，`plan-budget.mjs` 提供预算守卫（B1 实测 1407 行）
- **未做（后续）**：B9 真实构建 + 重启演练；n8n `approve` 未写 deploy 事件、未调 `notify_user`；执行沙箱容器化；A2A（P3）；harness「单批范围守卫」（批内 >10 文件自动暂停）
- **详见**：`decisions/ADR-0013-n8n-langgraph-deepagents-layering.md`、`ADR-0014-dev-chain-task-queue-and-phased-plan.md`、`ADR-0015-dev-line-mcp-bridge-persistence-delta.md`、`plan/01a15977-2e23-712c-8f0b-46fda48be9c0.md`、`docs/modules/dev-task.md`

## M15：移动端形态定案 + 交互原语分级（ADR-0011 / ADR-0012）✅

- **日期**：2026-10-19（文档决策，无代码改动）
- **完成**：
  - **ADR-0011 移动端形态**：短中期**不建原生 App**（交付物 = 现有 web 页 + PWA 化：manifest / 图标 / `display: standalone`）；需要独立图标/启动器时选 **Tauri 2**（Android 先行、壳加载线上页、**不启用 IPC**）；**RN/Expo 延后**，并列出 5 条重开条件（离开 App 的通知 / 原生级交互 / 相机扫码等硬件 / iOS 上架且无 macOS / web 迁 React）；正式取代 ADR-0010 里「按 RN 复用」的前提措辞
  - **ADR-0012 交互原语**：三轴分级 —— 基数（`one` / `many` / `partition`）× 可见性（`plain` / `masked`）× 步数与主体（单步 / **级联**同主体连续 / **接力**跨主体 / 异步等待）；引擎只声明**结构语义**，控件由消费端决定；**Q 群消息侧冻结**（编号清单 + `#选 N` 不再改）；规范正文 `docs/conventions/interaction-primitives.md`
  - 判据沉淀：区分「同一人连续多步」（F2/H2/F1/H1）与「跨人接力」（A1/G1 下注者→左手玩家→赢家），避免端侧误做「一次提交」
- **验证**：仅文档/决策（提交 `b7d2a65`、`8e9e831`、`ddfa5aa`）；跨端基础有 `test-view-rn.mts`（222 图元契约守卫）兜底
- **未做**：PWA 化（manifest / 图标 / standalone）尚未落地；RN 工程不建；离 App 通知等原生能力延后
- **详见**：`decisions/ADR-0011-mobile-app-form-pwa-tauri2-vs-rn.md`、`ADR-0012-interaction-primitives.md`、`docs/conventions/interaction-primitives.md`、`docs/modules/web.md`

## M14：版图页双模式 + 素材流水线（ADR-0010）✅

- **日期**：2026-10-18~19（2026-10-19 两轮人工验收）
- **完成**：
  - **渲染层**：新增 `apps/web/src/view/`（纯 TS：`types/format/text/board{theme,scene,svg,manifest}/page`），服务端 SSR 首屏 + esbuild 浏览器 bundle（≈20KB）**复用同一套纯函数**；**不引入 React/Vite**
  - **`BoardScene` 场景 IR** 作为跨端唯一契约（5 类图元 / 绝对坐标 / 无平台属性）
  - **页内双模式**：默认**版图**、`#text` 切文本，链接与 token 语义不变；`GET /tabletop_game/{gameId}/state` + 10s 轮询 + 激活立即刷新 + 失败退避 + 无 JS 降级
  - **版图内容**：六边形赌场盘（矢量 + 实物底盘 0.5 透明贴图）+ 6 张赌场牌 + 玩家面板 + 公共区 + 终局覆盖；16 块小游戏大图当版面、文字层/状态标记按实物印位压图、**全部顺序平铺**（不分页）；加密选项在版图态同样可见（`renderSecretOptions` 文本/版图共用）
  - **素材流水线**：`apps/scripts/board-assets.mjs`（inventory/build/preview）+ `board-hex.mjs`（底盘标定）；16 张板块按引擎名 `A1-…H2` 命名（id 由文件名解析）；抠图/纠偏/紧裁/压缩后合计 ≈317KB；**ComfyUI（SD1.5-inpainting）仅离线修图，不参与实时渲染**
- **验证**：`npm run test:web` 三套全绿（test-render / test-view / test-view-rn 222 图元）；server 3000 / web 18080 / 公网 tunnel 均 200；人工验收（版图态 M1–M3 → 整体收口）
- **关键结论**：版图坐标以**紧裁后的板块图**为基准（`TILE_SLOTS` 锚点微调只需刷新）；「4+ 块小游戏分页/折叠」裁定**作废**；底盘霓虹店名**保留**（不做 inpaint）；自动 inpaint/QC 脚本方案改为人工跑 ComfyUI 遮罩工作流（一次性任务）
- **未做**：板块锚点等实际对局反馈再微调；RN 工程接入（按 ADR-0011 延后）
- **详见**：`plan/01a14f46-64df-76b7-ab25-1c6cc8189d2a.md`、`docs/modules/board-view.md`、`docs/games/las-vegas/props/{board-layout,asset-mapping}.md`、`decisions/ADR-0010-board-rendering-dualmode-assets.md`

## M13：主动消息门禁 + 动作反馈合并单条（ADR-0009）✅

- **日期**：2026-10-17
- **完成**：
  - **主动消息默认禁用**：被动回复超限（同一条消息第 6 次起）默认**丢弃并告警**，仅 `PROACTIVE_MESSAGES_ENABLED=1` 才降级为主动消息
  - **开发门禁**：方案含主动消息（`pushActive` / 无 `reply_msg_id` / 文案提「主动消息」）必须 ① 在「待确定事项」列用途与配额影响 ② **人工确认** ③ 计划文档写标记 `【主动消息已人工确认】` ④ `npm run check:proactive-gate` 通过；plan-workflow skill 列为硬性约束
  - **门禁脚本 + 负样本单测**：`apps/scripts/check-proactive-gate.mjs`（根/apps 双入口）+ `test-proactive-gate.mts`（正文/引用/行内代码/代码块里提及**不算**确认；标记须单独成行）
  - **配套出站策略**：每次动作的「模板 + LLM 叙事」**合并为一条**被动回复，机器人动作按回合聚合，避开「每条消息 5 次被动回复」上限
- **验证**：`test-proactive-gate` 通过；后续各计划（含 M16）门禁实际拦截/放行正确（2026-10-21 修复脚本两处缺陷：取最新计划 + 标记须独立成行）
- **详见**：`decisions/ADR-0009-proactive-message-gate.md`、`docs/conventions/coding-standards.md`、`docs/conventions/external-constraints.md`

## M12：拉斯维加斯引擎落地 + 16 块小游戏对齐（ADR-0008）✅

- **日期**：2026-10-16~18（16 块对齐完成于 2026-10-17~18）
- **完成**：
  - **框架层（ADR-0008）**：`EngineEvent` 事件序列 + `ActionResult.events`（逐条 append、快照 `version` = 最后事件 seq）；`RandomSource` 注入（`SeededRng` 确定性 / `SystemRng` 默认）；lobby 生命周期统一走 `execute_action("lobby:*")`；`public_state` / `player_view` 视图脱敏 + `get_player_view` 原语；**群内唯一活跃对局**（含 lobby 时拒绝新建）
  - **引擎**：`crates/engines/las-vegas` 完整三阶段流程（掷骰 → 下注 → 每轮小游戏 → 结算）；**16 块小游戏 A1–H2 全部对齐**；引入**声明式结算时机**（`sg::Tempo` 10 类 + `tempos_of` 派发 + `tempo_table_covers_all_tiles` 快照守护）；状态 schema 演进到 v6
  - **交互能力**：加密选项通用化（A1 单选 / G1 多选，代码↔真义随机置换，公共视图抹 `perm`/`count`）；`resume`/`hint` 恢复原语（不改状态、随消息重签版面页 token）；**pending 队列**避免同次下注的多个交互互相吞掉
  - **群侧命令**：`#单人`（可直接指定开局小游戏）、`#放`/`#重投`/`#选 N`/`#动作 <id>`/`#恢复`/`#提示`/`#小游戏#xx`
  - **文档**：`rules.md`、`legal-actions.md`、`small-games-overview.md`（16 块速查总览）、`small-games-alignment.md`（对齐台账）、`balance-evaluation.md`（奖励期望值）
- **验证**：`cargo test -p las-vegas` 31/31 → 32/32（含 self_play 确定性、4 人完成、启发式优于随机、5 个跨板块集成用例）；`cargo test --workspace` 全绿无警告；MCP 冒烟 47–132 步通过；实测修正若干规则偏差（G1 改每轮结算前钩子、H1 改下注后两段式 + 每回合最多移 1 颗、E2 补派发、H1 改为下注到 H1 赌场后才触发）
- **关键结论（平衡）**：期望值 A2 1.58 万/次、B2 最优 1.5 万/次、E1 1.0 万、G1 5 万/轮、H1 **6 万/次（偏强）**、H2 3.8 万、A1 2.0 万 → 优先处理 H1 与 F2 a 组；村规确认三项（F2 灰骰 1–3 各 1 / 4–6 各 2、F1 两段式、H2 4 点重掷两骰）
- **未做**：H1 偏强的平衡调整；F2 a 组奖励偏弱
- **详见**：`plan/01a145b5-969e-778c-95d5-b212de7955fe.md`、`plan/01a14841-e081-7428-a966-f3d8caecfa78.md`、`docs/games/las-vegas/*`、`decisions/ADR-0008-engine-events-rng-lobby.md`

## M11：会话隔离 + 全局人物画像层（ADR-0007）✅

- **日期**：2026-10-16
- **完成**：
  - **chat 会话按 `(group_id, openid)` 隔离**（Redis `chat:history:{group}:{openid}` / `chat:boundary:{group}:{openid}`）；组装前**按 `boundary` 裁到当前阶段（a1）**（`chat-history.ts::cropToCurrentPhase`）
  - **game 局面摘要**：`game-engine-core` `EnginePlugin::summarize` + `SUMMARY_MAX_CHARS=300` + `GameManager::state_summary`；`game-engine-mcp` 新增 `state_summary` 原语；`narrateGame` 注入局面（+玩家画像）
  - **对话冷存**：agent-gateway chat/game 分支发 `chat.log` → `server` 消费落 `chat_history`（per-user，`event_id` 幂等）；`chat_history` 由群级迁移为 per-user
  - **全局人物画像**（跨群，`openid`）：`user_profile` 表 + `/internal/*` API；**日批蒸馏**（`PROFILE_DISTILL_HOUR=4` `CHAT_TZ`，增量游标）——**生成归本地小模型、把关归 SystemOne**；画像作全局层注入 chat/game
  - **本地蒸馏模型**：实测 9B 思考型不适合（想不停/复读）→ 改用 **Qwen2.5-3B-Instruct（few-shot 抽取）**；`llama-server` 按需起停 + **进程级释放确认**（PID/端口/健康）
  - 上下文来源**按线抽象**（`src/context.ts`）；文档同步（persistence / architecture / agent-gateway / game-engine / decision-service / mcp-primitives）
- **验证**：`tsc` 通过；`npm test` 15/15；`cargo test --workspace` 全绿；真机 bootstrap：全量 417 条 → 2 份画像落库并经 `/internal/profile` 读回；9B/3B 释放确认通过（显存回落基线）
- **未做（后续）**：非 @ 采集、群层记忆、会话摘要压缩（T3）、向量召回（T4）；未来「砍 chat + 放开非 @ → 两线合并」（`context.ts` 已留口子）
- **详见**：`decisions/ADR-0007-session-isolation-user-profile.md`、`plan/01a14523-d6ba-778c-95d5-b210991ba8e3.md`

## M10：chat 联网（Plan B 链接抓取）+ LLM 思考默认开启 + dev 子会话接入 MCP ✅

- **日期**：2026-10-15
- **完成**：
  - `apps/agent-gateway/src/chat.ts`：chat / game 叙述 thinking **默认开启**（`LLM_THINKING_ENABLED=1`，`callLlm` 不传 `thinking`；`=0` 显式关闭）——纠正如期（「被动回复 3s 超时」误判，实为 5 分钟）
  - chat 联网（Plan B）：仅消息含 `https://` 时触发 → SystemOne `noul`（是/否）判断是否抓取该链接 → 命中则 `fetch` 抽正文注入（`src/link-fetch.ts`：仅 https / 防 SSRF 字面量 / 手动跟随重定向逐跳校验 / 超时限长）；失败静默降级；判断命中与抓取成功均有日志
  - **意图分类前屏蔽 URL**：`decideIntent` 送模型的消息把 `http(s)://…` → `[链接]`（`maskUrls`），修掉链接文本（如 `/dev_docs`）把意图带偏为 dev、绕过 chat 抓取的问题
  - dev 子会话（`dev-session.ts`）接入 pi MCP：`createMcpExtension` + `codemode`/`tool_search` + `bindExtensions`，可用 playwright（`mcp__playwright__browser_*`）与 `bash`
  - 文档：`docs/architecture.md` §3.1 LLM 能力矩阵（思考/联网）+ agent-gateway/dev-task/decision-service 模块文档 + ADR-0002 修订
- **验证**：`tsc --noEmit` 通过；`npm test` 通过（新增 `test-link-fetch`，`test-decision` 补 `maskUrls` 断言）；真实决策服务实测：原误判消息 `chat 0.821`、英文版 `chat 0.824`、真 dev 带链接 `dev 0.951`；真机群消息端到端跑通（`intent=chat` → 链接抓取判断命中 → 正文注入 → 回复）
- **关键结论**：
  - 决策模型对 **URL 文本**高度敏感：含 `dev_docs` 的链接中英文均被判 dev（英文 0.90/0.97，比中文更极端）——**与语言无关，翻译无用**；屏蔽 URL 即解决
  - chat 抓取用**朴素 `fetch`**（轻量、单跳）；playwright 留给 dev 子会话（重、按需）
- **未做（后续）**：目标站点 JS 渲染/反爬时 `fetch` 抓不到 → 再评估 chat 改走 playwright；观察 chat thinking 的延迟/成本（必要时 `LLM_THINKING_ENABLED=0` 回退）
- **提交**：`579664c`、`1073132`

## M9：web 页呈现（代码链路 + cloudflared 公网可达打通）✅

- **日期**：2026-10-12
- **完成（代码侧）**：
  - `crates/auth`：短期 token 签发/校验/撤销（Redis 存储，uuid v4 + 绑定 openid + 资源范围，TTL 8h，局终态 `revoke_resource` 清空）；`cargo test -p auth` 8/8（含 Redis 集成：issue→verify / revoke / revoke_resource / 空 openid 拒绝）
  - `crates/server`：新增 `POST /auth/issue`、`POST /auth/verify`、`GET /resources/dev_task/{sessionId}/plan`（uuid 校验 + token 鉴权 + 资源范围匹配 → 401/403/404/400）、`GET /resources/game/{gameId}`（占位「暂未开放」）
  - `apps/web`：Express BFF（`/dev_task/{sessionId}?token=`、`/tabletop_game/{gameId}/player/{openid}` 占位、`/health`）+ markdown(GFM, marked) 渲染 + trust proxy
  - `apps/agent-gateway`：`auth-client.ts` 调 server 签发 token，拼短链替换 >1500 字计划长文推送（`PUBLIC_BASE_URL` 未配置退回长文）
  - 术语统一：openid（= member_openid，非 QQ 号）；路由/文档 `{qqid}`→`{openid}`
- **验证**：`cargo check --workspace` 通过；`cargo test -p auth` 8/8；server API 冒烟（issue→verify/401/403/400/占位）；web 本地 curl 冒烟（渲染/401/游戏占位）；agent-gateway `tsc` + `test:short-link` 通过
- **cloudflared（已完成）**：域名 `20221219.xyz` 切 Cloudflare NS；`cloudflared tunnel login/create/route dns` 本地管理隧道 `tabletop-web`（tunnel id `562666ab-...`）；Windows 服务常驻（`deploy/cloudflared/install-service.ps1`，ImagePath `tunnel --config ... run`）；公网 HTTPS `https://tabletopgame.20221219.xyz/health` → 200（本机大陆网络实测可达）
- **未做（后续）**：端到端 QQ 实测（#开发 → 计划生成 → 推短链 → 手机点开）；手机端大陆可达性复核
- **详见**：`plan/ad7f436a-1443-49ad-b229-220c44c17e80.md`

## M8：主动推送测试（push 单测 + qq.push 出站验证脚本）✅

- **日期**：2026-10-12
- **完成**：
  - `crates/push`：抽取 `ensure_event_id` 纯函数并补单测（event_id 自动补全 / 序列化 round-trip），`cargo test -p push` 3/3
  - `apps/verify-push.mjs`：注入一条「主动消息」指令（`reply_msg_id=null`）到 `qq.push`，观察 qq-gateway PEL/ACK 与消费者组信息，支持 `--no-cleanup` / `PUSH_TEST_GROUP`
  - `apps/qq-gateway`：`sendGroupMessage` 返回发送结果并在成功时记录 `msg_id`，便于验证主动/被动发送结果
- **验证**：`cargo test -p push` 3/3；`apps/qq-gateway` `tsc --noEmit` 通过；`verify-push.mjs` `node --check` 通过
- **未做（后续）**：真机向真实群发主动消息（需 Redis+QQ 在线+测试群 openid）；主动消息频控/预算记账（push.md 设计要求）；`at_member_id`(@提及) 出站使用、`PushEvent.msg_seq` 字段补齐

## M7：开发任务（多轮对话 + 子会话 plan-workflow 复用 + MCP 化）✅

- **日期**：2026-10-11
- **完成**：
  - `crates/dev-task`（core）：确定性状态机（queued→planning→awaiting_confirm→running→verifying→done/failed/cancelled）+ 白名单校验；`crates/dev-task-mcp`（stdio MCP）：9 个读写原语 + `list_capabilities`
  - `storage`：`dev_tasks` + `chat_history` 表落地
  - `agent-gateway`：`dev-task-client.ts`（MCP 客户端）、`dev-session.ts`（pi-sdk 子会话创建/恢复，复用 plan-workflow）、`dev-command.ts`（固定前缀命令硬判断）、消息桥（被动回执/状态同步 + 主动推送计划）
  - 聊天多轮：每群 Redis 热缓存最近 20 条（`CHAT_HISTORY_LIMIT` 可配）注入 `chatReply`
  - **关键修正**：MCP SDK `StdioClientTransport` 默认只继承白名单环境变量，`SQLITE_PATH`/`DEV_WHITELIST` 需显式 `env` 传入
- **验证**：Rust 单测 4/4；Node 命令解析单测；dev-task-mcp 集成验证；子会话 spawn 冒烟 + **计划工作流复用验证**（子会话生成 6 分区计划并落盘）
- **详见**：`plan/01a12ba0-c415-75b9-a5a5-1f74888e401d.md`
- **未做（后续）**：端到端 QQ 实测（需 Redis+LLM+QQ 在线）、web 页呈现（cloudflare tunnel）、聊天历史 SQLite 归档与 Redis 热缓存同步

## M6：决策模型接入（SystemOne 替代 mock 意图决策）✅（意图部分）

- **日期**：2026-10-11
- **完成**：
  - decision-service 部署（llama-server + gguf_server；一键启动 `apps/scripts/start-decision-service.mjs`）
  - agent-gateway 新增 `SystemOneClient`（HTTP `/v1/systemone`，超时可配/可注入）；`decideIntent` 改异步调真实模型，**mock 关键词彻底移除**
  - criteria 中文；双通道兜底：决策 → 低置信 `#前缀` → 默认 chat；服务不可用 → 「服务繁忙」
  - 配置：`SYSTEMONE_URL` / `SYSTEMONE_TIMEOUT_MS` / `DECISION_INTENT_THRESHOLD`
  - 单测改注入桩（离线）+ 真实服务集成验证脚本
  - 顺带修复既有前缀规则 bug（`#开发` 误判为 start_game）
- **验证**：`tsc` build 通过；`npm run test:decision` 通过；`npm run test:decision:live` 5/5；「这局规则是什么」→ chat(0.649)、「猜拳怎么玩」→ chat(0.822)，不再误判开桌
- **未做（后续）**：动作选择接入（架构 §6.1：先 `get_active_game`/`get_legal_actions` 再双问 `intent`+`action`）—— 留待拉斯维加斯引擎落地

## M5：游戏回复策略 C 方案 + 外部约束事实登记（被动窗口 5 分钟纠正）✅

- **日期**：2026-10-11
- **背景**：原计划的「M5 聊天增强 = 异步主动发送（LLM 不走被动窗口）」建立在「被动回复 3s 超时」的**误判**上；实测为 **5 分钟**窗口（每条消息最多 5 次，`msg_seq` 递增），整个 M5 因此改写
- **完成**：
  - **回复策略 C 方案**：游戏动作默认走**规则模板摘要**（承上启下、<1s、`msg_seq=1`）；`GAME_NARRATION_ENABLED=1` 时追加 LLM 情景叙述（`msg_seq=2`，失败/超时静默降级为仅模板）；LLM 只做「承上启下 + 情景带入」，**不逐条复述**结果（详情走 web 页）
  - 出站：`pushReply` 增加 `msg_seq` 参数；qq-gateway 从 push 事件读 `msg_seq` 并显式传给 `bot.send(extra)`，取代 `sendText` 的随机 `msg_seq`
  - **事实纪律**：新建 `docs/conventions/external-constraints.md`（外部约束登记表，AGENTS.md 索引、渐进披露）；`coding-standards.md` 增「事实 vs 假设」（数值约束须带来源 + 验证状态）；ADR 模板加「依据与验证状态」；plan-workflow skill 列为硬性约束；`apps/scripts/check-constraints.mjs` 扫描未标注来源的数值行
  - 文档同步：`architecture.md` §8/§6.3、`agent-gateway.md`、`decision-service.md`、ADR-0002 决策 10 修订
- **验证**：模板延迟 <1s；开关开启时叙事在 5 分钟窗口内送达；叙事不复述全部结果且限长；`rg` 复查 docs/.pi/memory_palace 无「不做 LLM 叙述」「威胁被动回复窗口」「3s 超时」残留；`cargo check` / `npm build` 通过
- **后续接口**：M10 在此基础上把 thinking 默认开启；2026-10-21 的「叙述主体 = 执行玩家」修复也在这条线上（`narrateGame` + `actor`）
- **详见**：`plan/01a12aaf-7246-75b9-a5a5-1f703bababad.md`、`docs/conventions/external-constraints.md`、`memory_palace/decisions/ADR-0002-dual-model-mcp-eventsourcing.md`（决策 10 修订）

## M4：QQ 群消息全链路 + MCP 统一原语 + 猜拳引擎 ✅

- **日期**：2026-10-11
- **完成**（P0→P3 四阶段，计划 01a12909）：
  - **P0 链路底座**：`shared` 定义 `InternalEvent` / `MessageGroupAt` / `PushEvent` + `event_id`；`bot-adapter` 用消费者组订阅 `qq.raw`（只留 @文本、trim、归一化为 `internal`）；`push` + `qq-gateway` 出站消费 `qq.push` → QQ API；`server`（Axum）+ `GET /health`
  - **P1 agent-gateway**：新建 Node/TS 编排者，消费 `internal` → 前缀解析 → mock 决策（intent+confidence）→ 交叉验证兜底 → chat 桩 → 写 `qq.push`；聊天链路端到端跑通
  - **P2 游戏/MCP**：`game-engine-core`（`EnginePlugin` 最小 trait + 注册表）；`game-engine-mcp` 统一原语（`get_rules` / `get_active_game` / `get_state` / `get_legal_actions` / `start_game` / `execute_action`，stdio）；`engines/rock-paper-scissors`（一轮猜拳：模拟决策解析手势、随机出拳、判定）；agent-gateway 接 MCP client（**不走 HTTP 直连**）
  - **P3 持久化与运维**：SQLite 落库 `group_messages`（进/出）+ `game_events` / `game_state`（**先写库再 ACK**）；重启后恢复悬挂对局（PEL 重投 + `restore`）；后台自检写 Redis `health:status` + `/health` 聚合；dev 白名单（`DEV_WHITELIST`）；docker-compose + 启动顺序文档
  - 同期接入 **DeepSeek LLM** 替代 chat 桩（+ 异常回退兜底）
- **验证**：聊天端到端、猜拳端到端（输/赢/平）、低置信前缀兜底可观测、MCP 原语经 stdio 调用正常、`/health` 反映依赖状态、SQLite 有对应记录、rps 引擎 +7 单测
- **未做**：`猜拳 ✌️` emoji 手势的真机用例（后续 `extractHand` 已支持 ✊/✌/✋ 与英文）；「自然语言 → mock 决策」两条人工用例未执行（M6 已整体换成 SystemOne，该验证口径作废）
- **详见**：`plan/01a12909-b4ec-75b9-a5a5-1f6fc331f2e7.md`、`docs/modules/{bot-adapter,game-engine,agent-gateway}.md`

## M3：架构重设计（双模型 + MCP 化 + 事件溯源，文档）✅

- **日期**：2026-10-11
- **完成**（仅文档，无业务代码）：
  - 引入双模型：主持人 LLM（远程 API，型号待定）+ 决策模型 SystemOne（StartLux-Decision-4B Q8_0 GGUF）
  - agent-gateway（Node, pi-sdk）为唯一编排者；command-router 职责并入
  - game-engine 以 MCP 形态暴露统一原语（stdio）；决策模型只建议不写状态
  - 权限/合法性由规则引擎确定性校验；置信度阈值配置化 + 兜底
  - 意图 + 动作合并为一次决策调用（单次前向多问题）
  - 事件溯源：`game_events` + `game_state` 快照；会话标识三类拆分
  - 分层回复策略（游戏模板摘要默认 + 可配置 LLM 情景叙述 / 聊天 LLM 叙述 / 开发回执+异步）
- **详见**：`decisions/ADR-0002-dual-model-mcp-eventsourcing.md`

## M2：qq-gateway 接入（群 → qq.raw 链路跑通）✅

- **日期**：2026-10-10
- **完成**：
  - 接入 QQ 官方 WebSocket 网关（官方 SDK `@tencent-connect/qqbot-nodejs`）
  - 接收 `GROUP_AT_MESSAGE_CREATE`（群 @消息）
  - 原始事件写入 Redis Stream `qq.raw`（`MAXLEN ~ 1000`，单字段 `event`）
- **验证**：群里 @机器人 → 日志 `→ qq.raw [GROUP_AT_MESSAGE_CREATE]`；`XRANGE qq.raw` 确认 XLEN=1
- **关键结论**（已补进 `docs/conventions/message-protocol.md`、`docs/modules/bot-adapter.md`）：
  - 玩家身份 = `member_openid`（**非 QQ 号**）
  - `content` 带前导空格，bot-adapter 需 trim
  - 被动回复用 `raw.id` 作为 `msg_id`
- **环境**：Docker Desktop（WSL2）+ Redis 7（镜像加速 `docker.m.daocloud.io`）

## M1：项目初始化（脚手架）✅

- **日期**：2026-10-08
- **完成**：目录结构、AGENTS.md 渐进披露、skills/rules、docs、Node/Rust 空骨架、拉斯维加斯空引擎
- **验收**：`npm install`、`cargo metadata`、`cargo check --workspace` 通过
- **详见**：`plan/01a11c33-43ef-7462-9d16-2ffb5822fe8a.md`

## 下一里程碑（待办）

- **ADR 状态收口**：ADR-0013/0014/0015 仍标「提案（Proposed）」，而 dev 线迁移已落地（M16）→ 应转 **Accepted** 并补落地证据；ADR-0016 尚不存在（新决策请按序新开）
- **B9 收尾**：n8n `approve` 写 deploy 事件 + 调 `notify_user`；真实构建 + 重启演练（当前只有 dry-run）
- **执行隔离**：dev 执行器沙箱容器化（另立 ADR）
- **拉斯维加斯**：16 块小游戏已对齐 → 真机整局验收 + 版图页移动端复核；**H1 奖励偏强（6 万/次）待平衡调整**、F2 a 组偏弱；板块锚点 `TILE_SLOTS` 等对局反馈再微调
- **移动端**：PWA 化（manifest / 图标 / `display: standalone`，ADR-0011 P0）尚未落地；Tauri 2 壳与 RN 均延后
- **叙述主体修复（2026-10-21）**：game 线 LLM 叙述已带「执行玩家」，待真机跑一局复核（首次玩家命令 / 机器人批次两种场景）
- **M11 遗留**：非 @ 消息采集、群层记忆、对话摘要压缩（T3）、向量召回（T4）
- **harness**：单批范围守卫（批内改动 >10 文件即暂停拆批）
- **延后**：A2A（P3+）、chat 改 playwright 抓取（仅当目标站点 JS 渲染/反爬时）
