# 全武将验证 · 机器证据（自动生成，勿手改）

> 生成：`apps/scripts/sgs-general-audit.mjs` · 时间：2026-11-11T12:42:34.948Z
> 只含**机械证据**（可达/不报错/守恒/pending/可复现），不含语义判定；
> 人工走查结论请写入 `docs/games/sanguosha/general-audit.md`。

- 范围：全量 147 将（147 将）
- seed=20261105 · repeat=1 · players=5 · maxSteps=12
- 名录：232 将（审计 147 / 排除 85）/ 288 技能
- 结果：触达 124 · 未触达 23 · 失败 0 · 阻塞 0

| 批次 | 武将 | code | 技能（类型） | 状态 | 触达 | 未触达 | 不变量 | 备注 |
|---|---|---|---|---|---|---|---|---|
| - | 曹仁 | sgs_hero_29 | 据守(normal)、解围(normal) | 触达 | jushou | jiewei | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jushou |
| - | 曹丕 | sgs_hero_43 | 行殇(normal)、放逐(normal)、颂威(lord) | 触达 | fangzhu | xingshang,songwei | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：fangzhu |
| - | 袁术 | sgs_hero_100 | 庸肆(locked)、伪帝(normal) | 未触达 | - | yongsi,weidi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（yongsi,weidi） |
| - | 周泰 | sgs_hero_210 | 不屈(locked)、奋激(normal) | 触达 | fenji | buqu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：fenji |
| - | 王基 | sgs_hero_362 | 奇制(normal)、进趋(normal) | 未触达 | - | qizhi,jinzou | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（qizhi,jinzou） |
| - | 严颜 | sgs_hero_400 | 拒战(normal) | 触达 | juzhan | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：juzhan |
| - | 王平 | sgs_hero_401 | 飞军(normal)、兵略(locked) | 未触达 | - | feijun,binglue | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（feijun,binglue） |
| - | 陆绩 | sgs_hero_402 | 怀橘(locked)、遗礼(normal)、整论(normal) | 触达 | yili | huaiju,zhenglun | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：yili |
| - | 孙亮 | sgs_hero_403 | 溃诛(normal)、掣政(locked)、立军(lord) | 触达 | chezheng,kuizhu,lijun | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：chezheng,kuizhu,lijun |
| - | 蒯越蒯良 | sgs_hero_404 | 荐降(normal)、审时(normal) | 触达 | shenshi | jianxiang | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：shenshi |
| - | 许攸 | sgs_hero_406 | 成略(normal)、恃才(normal)、寸目(locked) | 触达 | chenglue,shicai | cunmu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：chenglue,shicai |
| - | 卢植 | sgs_hero_407 | 明任(normal)、贞良(normal) | 触达 | mingren | zhenliang | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：mingren |
| - | 郝昭 | sgs_hero_408 | 镇骨(normal) | 触达 | zhengu | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zhengu |
| - | 陈到 | sgs_hero_409 | 往烈(normal) | 未触达 | - | wanglie | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（wanglie） |
| - | 诸葛瞻 | sgs_hero_410 | 罪论(normal)、父荫(locked) | 触达 | zuilun | fuyin | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zuilun |
| - | 周妃 | sgs_hero_411 | 良姻(normal)、箜声(normal) | 触达 | kongsheng | liangyin | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：kongsheng |
| - | 毌丘俭 | sgs_hero_413 | 征荣(normal)、鸿举(awaken) | 未触达 | - | zhengrong,hongju | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（zhengrong,hongju） |
| - | 陆抗 | sgs_hero_414 | 谦节(locked)、决堰(normal)、破势(awaken) | 触达 | jueyan | qianjie,poshi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jueyan |
| - | 张绣 | sgs_hero_415 | 雄乱(limited)、从谏(locked) | 触达 | xiongluan,congjian | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：xiongluan,congjian |
| - | 马谡 | sgs_hero_156 | 散谣(normal)、制蛮(normal) | 触达 | sanyao,zhiman | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：sanyao,zhiman |
| - | 荀攸 | sgs_hero_164 | 奇策(normal)、智愚(normal) | 触达 | qice,zhiyu | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：qice,zhiyu |
| - | 钟会 | sgs_hero_173 | 权计(normal)、自立(awaken) | 触达 | quanji | zili | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：quanji |
| - | 满宠 | sgs_hero_176 | 峻刑(normal)、御策(normal) | 触达 | junxing | yuce | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：junxing |
| - | 关平 | sgs_hero_177 | 龙吟(normal) | 触达 | longyin | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：longyin |
| - | 刘封 | sgs_hero_179 | 陷嗣(normal) | 触达 | xiansi | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：xiansi |
| - | 潘璋马忠 | sgs_hero_180 | 夺刀(normal)、暗箭(locked) | 未触达 | - | duodao,anjian | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（duodao,anjian） |
| - | 朱然 | sgs_hero_182 | 胆守(normal) | 触达 | danshou | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：danshou |
| - | 曹真 | sgs_hero_321 | 司敌(normal) | 触达 | sidi | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：sidi |
| - | 韩浩史涣 | sgs_hero_322 | 慎断(normal)、勇略(normal) | 触达 | shenduan | yonglue | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：shenduan |
| - | 陈群 | sgs_hero_323 | 品第(normal)、法恩(normal) | 触达 | pindi | faen | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：pindi |
| - | 吴懿 | sgs_hero_324 | 奔袭(locked) | 未触达 | - | benxi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（benxi） |
| - | 周仓 | sgs_hero_325 | 忠勇(normal) | 未触达 | - | zhongyong | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（zhongyong） |
| - | 孙鲁班 | sgs_hero_327 | 谮毁(normal)、骄矜(normal) | 触达 | jiaojin | zanhui | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jiaojin |
| - | 朱桓 | sgs_hero_328 | 奋励(normal)、平寇(normal) | 触达 | fenli | pingkou | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：fenli |
| - | 顾雍 | sgs_hero_329 | 慎行(normal)、秉壹(normal) | 触达 | shenxing,bingyi | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：shenxing,bingyi |
| - | 曹叡 | sgs_hero_332 | 恢拓(normal)、明鉴(normal)、兴衰(lord) | 触达 | mingjian,huituo | xingshuai | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：mingjian,huituo |
| - | 曹休 | sgs_hero_333 | 千驹(locked)、倾袭(normal) | 未触达 | - | qianju,qingxi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（qianju,qingxi） |
| - | 钟繇 | sgs_hero_334 | 活墨(normal)、佐定(normal) | 触达 | huomo | zuoding | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：huomo |
| - | 刘谌 | sgs_hero_335 | 战绝(normal)、勤王(lord) | 触达 | zhanjue | qinwang | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zhanjue |
| - | 张嶷 | sgs_hero_337 | 怃戎(normal)、矢志(locked) | 触达 | wurong | shizhi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：wurong |
| - | 孙休 | sgs_hero_338 | 宴诛(normal)、兴学(normal)、诏缚(lord) | 触达 | yanzhu,zhaofu | xingxue | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：yanzhu,zhaofu |
| - | 朱治 | sgs_hero_339 | 安国(normal) | 触达 | anguo | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：anguo |
| - | 全琮 | sgs_hero_340 | 邀名(normal) | 触达 | yaoming | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：yaoming |
| - | 公孙渊 | sgs_hero_341 | 怀异(normal) | 触达 | huaiyi | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：huaiyi |
| - | 郭图逢纪 | sgs_hero_342 | 急攻(normal)、饰非(normal) | 触达 | jigong | shifei | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jigong |
| - | 李严 | sgs_hero_381 | 督粮(normal)、腹鳞(locked) | 触达 | duliang | fulin | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：duliang |
| - | 孙登 | sgs_hero_382 | 匡弼(normal) | 触达 | kuangbi | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：kuangbi |
| - | 刘虞 | sgs_hero_383 | 止戈(normal)、宗祚(locked) | 触达 | zhige | zongzuo | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zhige |
| - | 岑昏 | sgs_hero_384 | 极奢(normal)、链祸(locked) | 触达 | jishe | lianhuo | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jishe |
| - | 孙资刘放 | sgs_hero_385 | 瑰藻(normal)、讥谀(normal) | 触达 | jiyu | guizao | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jiyu |
| - | 黄皓 | sgs_hero_386 | 寝情(normal)、贿生(normal) | 触达 | qinqing,huisheng | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：qinqing,huisheng |
| - | 张让 | sgs_hero_387 | 滔乱(normal) | 触达 | taoluan | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：taoluan |
| - | 辛宪英 | sgs_hero_388 | 忠鉴(normal)、才识(normal) | 触达 | caishi,zhongjian | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：caishi,zhongjian |
| - | 吴苋 | sgs_hero_389 | 福绵(normal)、怠宴(normal) | 触达 | fumian | daiyan | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：fumian |
| - | 徐氏 | sgs_hero_390 | 问卦(normal)、伏诛(normal) | 未触达 | - | wengua,fuzhu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（wengua,fuzhu） |
| - | 曹节 | sgs_hero_391 | 守玺(normal)、惠民(normal) | 触达 | huimin,shouxi | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：huimin,shouxi |
| - | 嵇康 | sgs_hero_392 | 清弦(normal)、绝响(normal) | 触达 | qingxian | juexiang | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：qingxian |
| - | 秦宓 | sgs_hero_393 | 谏征(normal)、专对(normal)、天辩(normal) | 触达 | zhuandui | jianzheng,tianbian | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zhuandui |
| - | 薛综 | sgs_hero_394 | 复难(normal)、诫训(normal) | 触达 | jiexun | funan | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jiexun |
| - | 蔡邕 | sgs_hero_395 | 辟撰(normal)、通博(normal) | 未触达 | - | bizhuan,tongbo | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（bizhuan,tongbo） |
| - | 界黄忠 | sgs_hero_26 | 烈弓(normal) | 未触达 | - | liegong | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（liegong） |
| - | 界刘备 | sgs_hero_299 | 仁德(normal)、激将(lord) | 触达 | rende | jijiang | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：rende |
| - | 界关羽 | sgs_hero_300 | 义绝(normal)、武圣(normal) | 触达 | wusheng,yijue | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：wusheng,yijue |
| - | 界张飞 | sgs_hero_301 | 替身(limited)、咆哮(locked) | 未触达 | - | tishen,paoxiao | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（tishen,paoxiao） |
| - | 界赵云 | sgs_hero_302 | 涯角(normal)、龙胆(normal) | 触达 | longdan,yajiao | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：longdan,yajiao |
| - | 界马超 | sgs_hero_303 | 马术(locked)、铁骑(normal) | 未触达 | - | mashu,tieqi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（mashu,tieqi） |
| - | 界徐庶 | sgs_hero_304 | 诛害(normal)、潜心(awaken) | 未触达 | - | zhuhai,qianxin | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（zhuhai,qianxin） |
| - | 界甘宁 | sgs_hero_305 | 奇袭(normal)、奋威(limited) | 触达 | qixi | fenwei | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：qixi |
| - | 界吕蒙 | sgs_hero_306 | 克己(locked)、勤学(awaken)、博图(normal) | 触达 | qinxue | keji,botu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：qinxue |
| - | 界黄盖 | sgs_hero_307 | 苦肉(normal)、诈降(locked) | 触达 | kurou | zhaxiang | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：kurou |
| - | 界周瑜 | sgs_hero_308 | 英姿(locked)、反间(normal) | 触达 | fanjian | yingzi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：fanjian |
| - | 界大乔 | sgs_hero_309 | 流离(normal)、国色(normal) | 触达 | guose | liuli | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：guose |
| - | 界陆逊 | sgs_hero_310 | 谦逊(locked)、连营(normal) | 触达 | lianying | qianxun | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：lianying |
| - | 界曹操 | sgs_hero_311 | 奸雄(normal)、护驾(lord) | 触达 | hujia,jianxiong | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：hujia,jianxiong |
| - | 界司马懿 | sgs_hero_312 | 反馈(normal)、鬼才(normal) | 触达 | fankui | guicai | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：fankui |
| - | 界夏侯惇 | sgs_hero_313 | 刚烈(normal)、清俭(normal) | 触达 | ganglie | qingjian | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：ganglie |
| - | 界张辽 | sgs_hero_314 | 突袭(normal) | 触达 | tuxi | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：tuxi |
| - | 界许褚 | sgs_hero_315 | 裸衣(normal) | 触达 | luoyi | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：luoyi |
| - | 界郭嘉 | sgs_hero_316 | 天妒(normal)、遗计(normal) | 触达 | yiji | tiandu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：yiji |
| - | 界李典 | sgs_hero_317 | 恂恂(normal)、忘隙(normal) | 触达 | xunxun,wangxi | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：xunxun,wangxi |
| - | 界华佗 | sgs_hero_318 | 急救(normal)、除疠(normal) | 触达 | chuli | jijiu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：chuli |
| - | 界吕布 | sgs_hero_319 | 无双(locked)、利驭(normal) | 未触达 | - | wushuang,liyu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（wushuang,liyu） |
| - | 界公孙瓒 | sgs_hero_320 | 义从(locked)、趫猛(normal) | 未触达 | - | yicong,qiaomeng | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（yicong,qiaomeng） |
| - | 界诸葛亮 | sgs_hero_440 | 空城(locked)、观星(normal) | 触达 | guanxing | kongcheng | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：guanxing |
| - | 界黄月英 | sgs_hero_441 | 集智(normal)、奇才(locked) | 未触达 | - | jizhi,qicai | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（jizhi,qicai） |
| - | 界孙权 | sgs_hero_442 | 制衡(normal)、救援(lord) | 触达 | zhiheng | jiuyuan | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zhiheng |
| - | 界孙尚香 | sgs_hero_443 | 枭姬(normal)、结姻(normal) | 触达 | jieyin | xiaoji | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jieyin |
| - | 界甄姬 | sgs_hero_444 | 倾国(normal)、洛神(normal) | 触达 | luoshen | qingguo | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：luoshen |
| - | 界貂蝉 | sgs_hero_445 | 离间(normal)、闭月(normal) | 触达 | lijian | biyue | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：lijian |
| - | 界华雄 | sgs_hero_446 | 耀武(locked)、势斩(normal) | 触达 | shizhan | yaowu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：shizhan |
| - | 界左慈 | sgs_hero_447 | 化身(normal)、新生(normal) | 触达 | huashen,xinsheng | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：huashen,xinsheng |
| - | 界张角 | sgs_hero_448 | 雷击(normal)、鬼道(normal)、黄天(lord) | 未触达 | - | leiji,guidao,huangtian | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（leiji,guidao,huangtian） |
| - | 界于吉 | sgs_hero_449 | 蛊惑(normal) | 触达 | guhuo | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：guhuo |
| - | 界袁绍 | sgs_hero_450 | 乱击(normal)、血裔(lord) | 触达 | xueyi,luanji | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：xueyi,luanji |
| - | 界刘禅 | sgs_hero_451 | 享乐(locked)、放权(normal)、若愚(lord) | 触达 | fangquan,xiangle | ruoyu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：fangquan,xiangle |
| - | 界孙策 | sgs_hero_452 | 制霸(lord)、激昂(normal)、魂姿(awaken) | 触达 | zhiba,jiang | hunzi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zhiba,jiang |
| - | 界夏侯渊 | sgs_hero_453 | 神速(normal)、设变(normal) | 触达 | shensu | shebian | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：shensu |
| - | 界卧龙诸葛亮 | sgs_hero_454 | 八阵(locked)、藏拙(locked)、火计(normal)、看破(normal) | 触达 | huoji | bazhen,cangzhuo,kanpo | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：huoji |
| - | 界庞统 | sgs_hero_455 | 连环(normal)、涅槃(limited) | 触达 | lianhuan | niepan | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：lianhuan |
| - | 界魏延 | sgs_hero_456 | 狂骨(locked)、奇谋(limited) | 触达 | qimou,kuanggu | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：qimou,kuanggu |
| - | 界小乔 | sgs_hero_457 | 红颜(locked)、天香(normal)、飘零(normal) | 触达 | piaoling,tianxiang | hongyan | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：piaoling,tianxiang |
| - | 界孙坚 | sgs_hero_458 | 英魂(normal)、武烈(limited) | 触达 | wulie | yinghun | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：wulie |
| - | 界庞德 | sgs_hero_462 | 马术(locked)、鞬出(normal) | 未触达 | - | mashu,jiandu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（mashu,jiandu） |
| - | 界太史慈 | sgs_hero_463 | 天义(normal)、酣战(normal) | 触达 | tianyi | hanzhan | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：tianyi |
| - | 界董卓 | sgs_hero_470 | 肉林(locked)、崩坏(locked)、酒池(normal)、暴虐(lord) | 触达 | jiuchi,bengkuai | roulin,baonue | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jiuchi,bengkuai |
| - | 界邓艾 | sgs_hero_471 | 屯田(normal)、凿险(awaken) | 触达 | tuotian | zaoxian | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：tuotian |
| - | 界祝融 | sgs_hero_479 | 巨象(locked)、烈刃(normal)、长标(normal) | 触达 | changbiao,liereng | juxiang | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：changbiao,liereng |
| - | 界姜维 | sgs_hero_480 | 挑衅(normal)、志继(awaken) | 触达 | tiaoxin | zhiji | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：tiaoxin |
| - | 界贾诩 | sgs_hero_484 | 帷幕(locked)、乱武(limited)、完杀(locked) | 触达 | luanwu | weimu,wansha | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：luanwu |
| - | 界鲁肃 | sgs_hero_485 | 好施(normal)、缔盟(normal) | 触达 | haoshi,dimeng | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：haoshi,dimeng |
| - | 界张郃 | sgs_hero_486 | 巧变(normal) | 触达 | qiaobian | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：qiaobian |
| - | 界孟获 | sgs_hero_492 | 祸首(locked)、再起(normal) | 触达 | zaizhi | huoshou | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zaizhi |
| - | 界徐晃 | sgs_hero_496 | 断粮(normal)、截辎(normal) | 触达 | duanliang | jiezi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：duanliang |
| - | 界蔡文姬 | sgs_hero_505 | 断肠(locked)、悲歌(normal) | 触达 | beige | duanchang | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：beige |
| - | 界荀彧 | sgs_hero_515 | 驱虎(normal)、节命(normal) | 触达 | jieming | quhu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jieming |
| - | 界典韦 | sgs_hero_526 | 强袭(normal)、狞恶(locked) | 触达 | qiangxi | ninge | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：qiangxi |
| - | 界颜良文丑 | sgs_hero_550 | 双雄(normal) | 触达 | shuangxiong | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：shuangxiong |
| - | 界张昭张纮 | sgs_hero_563 | 直谏(normal)、固政(normal) | 触达 | zhijian | guzheng | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zhijian |
| - | 界徐盛 | sgs_hero_587 | 破军(normal) | 未触达 | - | pojun | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（pojun） |
| - | 界凌统 | sgs_hero_588 | 旋风(normal) | 触达 | xuanfeng | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：xuanfeng |
| - | 界陈宫 | sgs_hero_597 | 智迟(locked)、明策(normal) | 触达 | mingce | zhichi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：mingce |
| - | 界曹彰 | sgs_hero_599 | 将驰(normal) | 触达 | jiangchi | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jiangchi |
| - | 界简雍 | sgs_hero_600 | 纵适(normal)、巧说(normal) | 触达 | qiaoshuo | zongshi_jie | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：qiaoshuo |
| - | 界虞翻 | sgs_hero_603 | 纵玄(normal)、直言(normal) | 触达 | zongxuan,zhiyan | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zongxuan,zhiyan |
| - | 界高顺 | sgs_hero_604 | 陷阵(normal)、禁酒(locked) | 触达 | jinjiu,xianzhen | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jinjiu,xianzhen |
| - | 界法正 | sgs_hero_610 | 眩惑(normal)、恩怨(locked) | 触达 | xuanhuo,enyuan | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：xuanhuo,enyuan |
| - | 界程普 | sgs_hero_620 | 疠火(normal)、醇醪(normal) | 触达 | lihuo | chunlao | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：lihuo |
| - | 界王异 | sgs_hero_621 | 贞烈(normal)、秘计(normal) | 触达 | zhenlie | miji | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zhenlie |
| - | 界张春华 | sgs_hero_625 | 绝情(locked)、伤逝(normal)、翦灭(normal) | 触达 | jianmie | jueqing,shangshi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jianmie |
| - | 界刘表 | sgs_hero_626 | 自守(normal)、宗室(locked) | 触达 | zishou,zongshi | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zishou,zongshi |
| - | 界蔡夫人 | sgs_hero_627 | 献州(limited)、窃听(normal) | 触达 | xianzhou | qieting | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：xianzhou |
| - | 界曹冲 | sgs_hero_628 | 称象(normal)、仁心(normal) | 触达 | chengxiang | renxin | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：chengxiang |
| - | 界曹植 | sgs_hero_629 | 落英(normal)、酒诗(normal) | 触达 | jiushi,luoying | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jiushi,luoying |
| - | 界吴国太 | sgs_hero_640 | 甘露(normal)、补益(normal) | 未触达 | - | ganlu,buyi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（ganlu,buyi） |
| - | 界廖化 | sgs_hero_641 | 当先(normal)、伏枥(limited) | 触达 | dangxian | fuli | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：dangxian |
| - | 界李儒 | sgs_hero_642 | 灭计(normal)、焚城(limited)、绝策(normal) | 触达 | mieji,fencheng | juece | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：mieji,fencheng |
| - | 界郭淮 | sgs_hero_658 | 精策(normal) | 未触达 | - | jingce | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | L1 未触达任何技能（jingce） |
| - | 界伏皇后 | sgs_hero_671 | 惴恐(normal)、求援(normal) | 触达 | qiuyuan | zhuikong | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：qiuyuan |
| - | 界韩当 | sgs_hero_676 | 弓骑(normal)、解烦(limited) | 触达 | gongqi,jiefan | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：gongqi,jiefan |
| - | 界关兴张苞 | sgs_hero_690 | 父魂(normal) | 触达 | fuhen | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：fuhen |
| - | 界马岱 | sgs_hero_696 | 马术(locked)、潜袭(normal) | 触达 | qianxi | mashu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：qianxi |
| - | 界郭皇后 | sgs_hero_726 | 矫诏(normal)、殚心(normal) | 触达 | jiazhao,danxin | - | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jiazhao,danxin |
| - | 界沮授 | sgs_hero_749 | 矢北(locked)、渐营(normal) | 触达 | jianying | shibei | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：jianying |
| - | 界夏侯氏 | sgs_hero_752 | 樵拾(normal)、燕语(normal) | 触达 | yanyu | qiaoshi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：yanyu |
| - | 界张松 | sgs_hero_753 | 强识(normal)、献图(normal) | 触达 | qiangshi | xiantu | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：qiangshi |
| - | 界于禁 | sgs_hero_756 | 镇军(normal)、毅重(locked) | 触达 | zhenjun | yizhong | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：zhenjun |
| - | 界步练师 | sgs_hero_775 | 安恤(normal)、追忆(limited) | 触达 | anxu | zhuixi | noError=✓ conservation=✓ pendingContinuable=✓ deterministic=✓ | 机械证据：anxu |

## 触达率（机器证据，供人工走查排序）

| 维度 | 技能数 | 触达 | 比例 |
|---|---|---|---|
| 批次 B0 | 7 | 3 | 43% |
| 批次 B1 | 18 | 8 | 44% |
| 批次 B10 | 17 | 8 | 47% |
| 批次 B11 | 14 | 10 | 71% |
| 批次 B12 | 8 | 2 | 25% |
| 批次 B13 | 16 | 8 | 50% |
| 批次 B14 | 20 | 10 | 50% |
| 批次 B15 | 19 | 11 | 58% |
| 批次 B16 | 14 | 7 | 50% |
| 批次 B17 | 14 | 11 | 79% |
| 批次 B18 | 17 | 9 | 53% |
| 批次 B19 | 15 | 10 | 67% |
| 批次 B2 | 15 | 7 | 47% |
| 批次 B20 | 8 | 4 | 50% |
| 批次 B3 | 9 | 7 | 78% |
| 批次 B4 | 11 | 5 | 45% |
| 批次 B5 | 13 | 7 | 54% |
| 批次 B6 | 12 | 8 | 67% |
| 批次 B7 | 12 | 8 | 67% |
| 批次 B8 | 17 | 8 | 47% |
| 批次 B9 | 14 | 6 | 43% |
| 类型 awaken | 8 | 1 | 13% |
| 类型 limited | 12 | 7 | 58% |
| 类型 locked | 49 | 8 | 16% |
| 类型 lord | 13 | 5 | 38% |
| 类型 normal | 208 | 136 | 65% |

