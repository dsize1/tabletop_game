# 术语表

| 术语 | 含义 |
|---|---|
| qq-gateway | QQ 官方协议接入服务（Node/TS） |
| agent-gateway | 主持人服务（Node/TS + pi-sdk），编排意图/决策/执行/叙述 |
| internal event | 内部统一消息事件（QQ 事件标准化后的形态） |
| qq.raw | Redis Stream：qq-gateway 写入的原始 QQ 群事件 |
| internal | Redis Stream：bot-adapter 写入、agent-gateway 消费的归一化事件 |
| qq.push | Redis Stream：agent-gateway/push 写入、qq-gateway 消费的推送指令 |
| EnginePlugin | game-engine-core 定义的引擎抽象 trait（产出事件序列 + RNG 注入 + public_state/player_view） |
| EngineEvent | 引擎产出的事件记录（event_type/actor/payload）；写入 game_events，快照 version 对齐 seq |
| RandomSource | 可注入随机源 trait（SeededRng 测试确定性 / SystemRng 默认） |
| Policy | 决策策略 trait（状态+合法动作 → 动作 id；测试 AI 与后续 AlphaZero 通用） |
| HeuristicPolicy | 拉斯维加斯启发式贪心 AI（按攻略「期望分/骰」基准 2 分打分） |
| 自对弈 / run_self_play | 用策略驱动一局（lobby→终局），固定种子可复现，供测试/回归 |
| 机器人补位 | `Seat.is_bot`；房主 `lobby:add_bot` 入局，内置启发式 AI 自动行动（`#单人`/`#加机器人`） |
| las-vegas | 首个桌游引擎「拉斯维加斯」 |
| 短期 token | Web 鉴权用的短时效 token：uuid v4 + 绑定 openid + 资源范围；存 Redis，TTL 8 小时（局终态清除） |
| openid（member_openid） | 玩家身份标识；QQ 官方仅给群内 openid（非 QQ 号）。统一术语：代码/文档以「openid」指代玩家身份，不再用「qq号/qqid」 |
| 主持人（LLM） | 远程 LLM（型号待定），负责叙述、闲聊、开发任务、编排 |
| SystemOne / 决策模型 | StartLux-Decision-4B（本地 GGUF），暴露 `/v1/systemone`，只建议不写状态 |
| 规则引擎 | game-engine 的确定性校验与状态机（合法性/权限/执行/事件溯源） |
| typed decision | 决策模型的问答类型：`choice`（多选一）/ `noul`（是/否）/ `score`（分级） |
| MCP 统一原语 | 各引擎标准化为 MCP 工具（只读查询 + 受校验写入） |
| 事件溯源 | append-only `game_events` + `game_state` 快照（version 对齐） |
| 置信度 | TypeSafe 公式 `(p_max − 1/n) / (1 − 1/n)`，用于阈值兜底 |
| wide_choice | 决策模型单题超 26 选项时的两阶段分组机制 |
| 被动回复 / 主动发送 | QQ 消息回复两种方式：带 msg_id 回复（群聊 5 分钟、每条最多 5 次）/ POST 主动推送（群 1000 条/天） |
| casino / 赌场 | 拉斯维加斯编号 1–6 的落骰/堆钞牌 |
| banknote / 钞票 | 拉斯维加斯奖池货币，面额 $3–$10 万（张数 11/11/13/15/13/11/9/7，共 90 张），每轮重洗抽 6 组×2 张；结算前背面朝上隐藏；终局按总金额计分（1 筹码=1 分=1 万） |
| chip / 筹码 | 拉斯维加斯资源货币；初始 2/人、每轮 +2；终局 ×$1 万计分；可用于支付弃回合/小游戏 |
| big die / 大骰 | 每人 1 枚；**仅赌场**中 1 大骰 = 2 小骰，其他情况（酒吧/小游戏区/金框/游戏场）一律算 1 |
| small die / 小骰 | 每人 7 枚；投掷后按点数整组下注 |
| gray die / 灰色骰 | 视为虚拟玩家，参与「移除平手」「权力优势」；由 F1/F2 小游戏决定初始摆放 |
| black die / 黑色骰 | 触发类小游戏投掷用（每次 2 颗）：A2/B1/B2/H2 |
| small game / 小游戏 | A1–H2 板块，每轮重新 16→8→N（N 默认 3，房主可设 3–6）抽 N 块，从 1 号赌场起依次挂到 1–3 号；对应赌场下注后可激活 |
| lobby / 游戏设置阶段 | 开赛前阶段：任何人可发起，群成员 @bot 加入/退出，发起人开赛；开赛后不可退 |
| host / 房主（发起人） | 创建对局者（自动入局）；唯一有权 `start` 开赛、`abort` 中断结束（不结算）、`kick` 踢人 |
| abort / 中断结束 | 房主中止对局：`aborted`，**不结算**，直接终结该群对局生命周期 |
| kick / 踢人 | 房主/发起人在 `lobby` 阶段移除某玩家 |
| round / 轮 | 共 3 轮；每轮重设钞票/抽小游戏/每人 +2 筹码/重定起始玩家 |
| seat / 座位 | 对局玩家顺序（= GameState.players 顺序） |
| small_remaining / 手中小骰 | 玩家尚未掷出的 7 枚小骰中剩余数 |
| big_remaining / 手中大骰 | 玩家尚未掷出的 1 枚大骰 |
| roll / 本轮掷骰 | 当前玩家回合开始时掷出的点数（小骰数组 + 大骰点数），合法动作据此派生 |
| place:n | 拉斯维加斯玩家动作 id：把本轮掷出的点数 n 骰子全部放到 n 号赌场 |
| reroll | 支付 1 枚筹码，收回骰子并跳过本回合 |
| remove-tie / 移除平手 | 钞票分配前剔除数量一样多的骰子；其玩家不参与该赌场分配 |
| 游戏场 / game field | G2 双倍颁奖使用的公共结算区（先移除平手再比数量） |
| 金框 / gold frame | H2 的 6 点奖励区，结算时仍在框内得 $6 万 |
| 酒吧 / bar | E2 的区域，骰子不参与钞票分配，大骰算 1 |
| phase / 阶段 | 引擎私有状态机阶段（lobby/setup/turn_roll/small_game/round_end/settling/finished/aborted） |
