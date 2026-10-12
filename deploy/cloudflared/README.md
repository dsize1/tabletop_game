# cloudflared 本地管理隧道部署

本目录是 web 页公网可达的隧道配置（本地管理隧道，**不需要 Cloudflare Zero Trust / 银行卡**）。

## 公网地址

```
https://tabletopgame.20221219.xyz  →  http://127.0.0.1:18080（apps/web）
```

## 完整搭建步骤（已踩坑记录）

```bash
# 1. 登录（浏览器授权，生成 ~/.cloudflared/cert.pem）
cloudflared tunnel login

# 2. 建隧道（生成 ~/.cloudflared/<tunnel-id>.json 凭证）
cloudflared tunnel create tabletop-web

# 3. 写配置（本目录 config.yml.example 为模板；服务版放 C:\ProgramData\cloudflared\config.yml）
#    关键：ingress hostname → 127.0.0.1:18080 + catch-all 404

# 4. 自动建 DNS CNAME（写入 Cloudflare DNS 区）
cloudflared tunnel route dns tabletop-web tabletopgame.20221219.xyz

# 5. 前台验证
cloudflared tunnel --config config.yml run tabletop-web
```

## Windows 服务常驻（踩坑记录）

`cloudflared service install` **不带参数**（带参数会被当成 token 报 base64 错），但新版装出的服务 ImagePath 只有裸 `cloudflared.exe`（不带 `tunnel run`），隧道不会跑。正确做法见 `install-service.ps1`：

1. 把 config.yml + 凭证放到 `C:\ProgramData\cloudflared\`；
2. 停服务后**直接改注册表 ImagePath**（`Set-ItemProperty`，别用 `sc config`——嵌套引号会传崩）：

   ```
   ImagePath = "C:\Program Files (x86)\cloudflared\cloudflared.exe" tunnel --config C:\ProgramData\cloudflared\config.yml run
   ```

3. 启动服务。

一键：以管理员运行 `install-service.ps1`。

## 安全约定

- `credentials.json` / `cert.pem` / `config.yml`（含本机绝对路径）**不入库**，已在 `.gitignore` 忽略。
- 应用层鉴权不变：短期 token + server auth；隧道只解决「可达」。
