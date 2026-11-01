# ADR-0031：Web 跨端确认权鉴（放弃 QQ 扫码登录）+ 中台白名单准入

- **状态**：已采纳（2026-11-01，用户逐条确认）
- **相关**：ADR-0020（多端 Web 优先）、ADR-0015（dev 链路白名单/单写者）、`docs/modules/auth.md`、`docs/modules/web.md`、`docs/modules/props-admin.md`、`docs/conventions/external-constraints.md`、`docs/games/sanguosha/development-plan.md`（P6 阻碍项、P7）

## 1. 背景

三国杀的对局在 Web 完成，Q 群只做开局/状态/链接。原计划用 **QQ 扫码登录** 把「Web 访客」认证为「群玩家」，实测与查证后不可行：

| 事实 | 来源 | 验证状态 |
|---|---|---|
| QQ 机器人群/单聊事件 `User` 含 `union_openid`（跨应用统一 OpenID，「可能为空」），旧文档要求「特殊关联应用 + 联系平台运营申请配置」 | `bot.q.qq.com` 事件与 User 模型文档 | 已验证（文档） |
| 本项目真实群事件 375 条，`union_openid` **全部为空** | `data/tabletop.db` 实测 | 已验证（实测） |
| QQ 群侧无成员信息接口（拿不到成员级 union 映射） | `server-inter/group/manage/` 为空白页 | 已验证（文档） |
| QQ 互联「网站应用」扫码登录可用，但**网站与回调地址须工信部备案** | `wiki.connect.qq.com` 应用审核规范 | 已验证（文档） |
| QQ 互联审核驳回明细含「**在线游戏应用暂不合作**」 | 同上 | 已验证（文档） |
| unionID 取用需邮件申请权限（100048） | `wiki.connect.qq.com` 概念和术语 | 已验证（文档） |

结论：扫码登录是**外部审批串联**（平台运营打通 `union_openid` + 互联应用审核 + 域名备案），且桌游站有明确驳回风险；本项目当前**无法备案**。

## 2. 决策

**放弃扫码登录，改为系统自建「跨端确认」权鉴**：Web 端用「注册码」证明身份记忆，Q 群端用 `#验证码#NNNN` 证明群身份控制权，两端口令同源（同一会话）才发放正式 token。

### 2.1 会话模型（同生命周期）

> 短效登录 token + 状态 + 4 位验证码 + 目标路由 = **一条 Redis 记录、同一 TTL（10 分钟，阶段转换时续期）**。

- 阶段：`need_register → need_login → code → confirmed`；`mode=reset` 复用同一状态机（确认后不跳转）。
- `target` 存在会话里，登录页确认后跳到该站内路由（游戏个人页 / `/props-admin/`）——因此登录链接与游戏链接**同源同前缀、可公网访问**。
- 确认后**一次性换票**：发 8h token 并置 `consumed`；二次访问返回 `used`（有 Cookie 直进 target，否则回群重取）。

### 2.2 注册码与验证码

- 注册码 **6 位 `[0-9a-zA-Z]`**，**Argon2id** 加盐哈希存 `web_accounts`（键 = `(group_id, member_openid)`），无明文字段。
- **R1**：注册码**只在群内验证码确认成功后**才落库/更新 → 错链注册不污染目标账号。
- **R2**：4 位码与**回复人 openid** 比对，这就是「本次登录/注册有效性」的判定；不匹配只回「不是你的链接」并给回复人重签自己的链接。
- 失败锁定：注册码错 **3 次 → 30 分钟**；错误文案统一（不区分未注册/密码错）。

### 2.3 token 传递

- 正式 token **只经 `Set-Cookie`（HttpOnly / SameSite=Lax / 8h）** 下发，URL 不再携带；`?token=` 保留给内部脚手架（`sgs:web-room`、MCP 调试）。
- Cookie 分域：`tg_game`（游戏页/动作）、`tg_props`（中台）。

### 2.4 中台准入（P1 边界变更）

- `/props-admin` 由 web **同源托管**、公网可达；准入 = 登录会话 + **`PROPS_ADMIN_WHITELIST`（默认回落 `DEV_WHITELIST`）**，非白名单 403。
- `/internal/gameprops/*` **不可被浏览器直连**：由 web 代理，代理前校验会话；server 侧再按「共享 token 或白名单 actor」二次判定（纵深）。
- `PROPS_ADMIN_TOKEN` 降级为 web→server 内部凭证，浏览器不再持有。
- 守护脚本 `check:props-admin-boundary` 口径改为：同源公网 + 白名单准入 + 内部 API 不直连。

## 3. 备选方案与放弃理由

| 方案 | 结论 |
|---|---|
| QQ 扫码登录（QQ 互联） | 外部审批串联 + 「在线游戏不合作」+ 无法备案 → 放弃（保留数据预留） |
| 只发 URL token（现状） | 链接转发即冒名；无「群身份」证明 → 放弃 |
| 私聊（C2C）投递链接 | 需先把 C2C `user_openid` 绑定到群 `member_openid`，仍需群内码，链路更长 → 未采用 |
| 群内一次性登录码（无 4 位码确认） | 码在群里可见，他人可抢登 → 加上「4 位码 + 回复人 openid 比对」 |

## 4. 数据预留（为将来扫码登录留门）

- bot-adapter 保留 `union_openid` / `member_role`，落 `member_identities`。
- server 预留 `/auth/web/qq/start|callback` 骨架与 `WEB_QQ_LOGIN_ENABLED=0` 开关；外部打通后按「同一 `openid → 身份` 抽象」替换。
- 探针 `apps/scripts/check-union-openid.mjs`：统计真实事件中非空 `union_openid` 比例。

## 5. 影响

- **P6 阻碍项**：机器人自动推进/身份校验依赖「Web 动作的主体可信」，本 ADR 是其前置（先完成）。
- **P7**：Q 群只做开局设定 + 极简局面播报 + 登录链接；群里不做逐动作交互（优先级最低）。
- **P8**：权鉴加固（动作级校验、审计、灰度）在本基础上继续。
