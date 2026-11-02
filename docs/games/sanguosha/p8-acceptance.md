# 三国杀 P8 上线验收清单

> 目标：权鉴、群管理控制、观测、回滚、真实群对局验收。
> 用户裁定（2026-11-03）：**不设灰度维度白名单**；加入本局座位的群成员可操作自己回合；
> 房主/群管理员（QQ `member_role=owner/admin`）可控制开局管理动作。

## 1. 权鉴（自动 + 人工）

- [ ] Web 动作：token 的 openid 必须等于动作 player_id（web 侧已如此，跨房间 403）
- [ ] 非当前行动者用本房间合法 token 提交动作 → 403/400（`test-sgs-web` 已覆盖）
- [ ] 已阵亡座位 / 非本局 openid 提交 → 403
- [ ] `/internal/game/*`、`/internal/metrics` 仅 127.0.0.1（外网不可达）
- [ ] 中台/预览（P5 后）只走 `props_admin` 会话 + 白名单
- [ ] 个人视图只返回本人手牌；公共视图 `player_area=null`、座位无 `hand`

## 2. 群管理权限

- [ ] 房主可 `#开始` / `#中断` / `#踢` / `#加机器人` / `#武将包`
- [ ] 群管理员（owner/admin，非房主）可代房主执行以上管理动作（以 host 身份提交，引擎权限不变）
- [ ] 普通群成员只能 `#加入` / `#退出`（lobby）/ 操作自己的 Web 回合
- [ ] 开赛后 `#退出` / `#踢` 回「请打完或房主/群管理 #中断」
- [ ] `SGS_ROLLOUT=off` 时新开三国杀被拒；进行中对局可打完/中断

## 3. 观测与回滚

- [ ] `GET /internal/metrics` 返回 `actions_total/actions_denied/games_started/games_finished/bot_steps/drives/decision_logs/decision_log_failures`
- [ ] 日志含 `room_id/seq/actor`，机器人推进有 `bot_stop`
- [ ] `npm run sgs:replay -- --room <room>` 对已结束局 `mismatches=0`
- [ ] 按 `docs/ops/sanguosha-runbook.md` 完成一次「中断→恢复链接→续推→replay」演练
- [ ] `npm run check:props-boundary` / `npm run check:proactive-gate` 通过

## 4. 真实群对局（人工，≥10 局 5 人 + 1 局 8 人）

| # | 人数 | room_id | 终局 | 严重规则错误 | 卡死/非法 | 备注 |
|---|---|---|---|---|---|---|
| 1 | 5 | | | | | |
| 2 | 5 | | | | | |
| 3 | 5 | | | | | |
| 4 | 5 | | | | | |
| 5 | 5 | | | | | |
| 6 | 5 | | | | | |
| 7 | 5 | | | | | |
| 8 | 5 | | | | | |
| 9 | 5 | | | | | |
| 10 | 5 | | | | | |
| 11 | 8 | | | | | |

- [ ] 10 局 5 人 + 1 局 8 人完成；无严重规则错误；无卡死/非法动作
- [ ] 回放可复现（至少抽 2 局 `sgs:replay`）
- [ ] 权鉴用例通过；无隐藏信息泄露；无主动消息
- [ ] 问题清单记录 room_id + 复现步骤；阻断类立即修，其余进 `general-audit.md`

## 5. 自动门禁

```bash
cargo test -p sanguosha -p storage -p server -p game-engine-mcp
npm run test:agent-gateway
npm run sgs:check
node apps/verify-sanguosha.mjs
npm run check:props-boundary
npm run check:proactive-gate
```
