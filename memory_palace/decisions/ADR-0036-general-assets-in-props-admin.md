# ADR-0036：武将卡面「入中台」——内容寻址素材库、缩略图/裁剪与公网可见边界

- 状态：已采纳（2026-11-04）
- 关联：ADR-0010（版图素材加工）、ADR-0017（中台数据模型）、ADR-0031（中台准入）、`docs/games/sanguosha/props/media-pipeline.md`

## 背景

素材流水线已产出 534 张 512×720 武将卡面（`data/media/sgs/out/final/`，覆盖 696 个 gid），但中台只有 `prop_assets` 表与 `image_asset_id` 空字段：没有上传、缩略图、裁剪、绑定与展示链路。需要在不破坏「server 单写者 / 浏览器不直连 server / 中台白名单准入」的前提下把卡面接进中台与对局页。

## 决策

1. **内容寻址素材库**：`prop_assets.path` 存相对路径 `{game_id}/{kind}/{sha256}.{ext}`，缩略图 `<sha256>.t.{ext}` 同行记录（`thumb_path/thumb_width/thumb_height` 幂等增列）。磁盘路径**只由 sha256 + MIME 派生**，绝不使用客户端文件名；`original_name`/`crop_json` 仅审计。
2. **单写者 + 同源静态服务**：server 独占写 `GAMEPROPS_ASSETS_DIR`（默认 `data/gameprops/assets`，已被 `/data/` 忽略）；web 以 `/game-assets` 静态挂载同一目录，文件名带内容哈希 → `immutable` 一年。浏览器仍不直连 server。
3. **裁剪/缩略图在浏览器完成**：props-admin 用 Canvas 裁剪（平移/缩放，固定 512×720），导出 WebP(q0.82) + 256×360 缩略图（不支持 WebP 编码时回退 PNG）；server 只做 MIME/尺寸/大小校验与落盘，不引入 Rust 图像依赖。
4. **绑定只写 `image_asset_id`**：新增 `POST /internal/gameprops/bind-asset`（校验表/素材存在，支持解绑），不复制业务行。`asset-map` 投影 `code → {url, thumb, width, height}` 供 web SSR。
5. **批量导入按 gid**：`final-manifest.json` 的 `gids` 与 `heroes.json` 全量一致（696/696），脚本 `import-sgs-general-assets.mjs` 按其绑定 `sgs_generals.code = sgs_hero_<gid>`，幂等可重跑。
6. **公网可见边界**：对局页仍需登录 token；图片 URL 本身公开但哈希不可枚举。卡面来源为 1994 剧照 + 生成图，**当前仅内部开发使用**，上线前须确认授权或替换（沿用 `media-pipeline.md` 的版权口径）。

## 理由与备选

- **备选 A（弃）**：web 逐图代理到 server。多一跳、缓存与并发都更差，且没有额外安全收益（server 只监听回环，文件本身公开）。
- **备选 B（弃）**：把素材写进 `apps/web/public/`。违反「server 单写者」，部署时两进程目录耦合更隐蔽。
- **备选 C（弃）**：Rust 端解码/裁剪（`image` crate）。只为了缩略图引入编解码依赖与 WebP 支持不确定性；浏览器 Canvas 已足够且交互更好。
- **删除保护**：素材被 `sgs_generals/sgs_card_defs/sgs_cards/sgs_identities` 引用时拒绝删除（409），避免线上裂图。

## 影响

- 新增 `prop_assets` 列（旧库幂等 `ALTER TABLE`）、server 素材路由（`gameprops_assets.rs`）、web `/game-assets` 静态路由与 `asset-map` 缓存（60s）、props-admin「素材」页 + 素材选择/裁剪上传、批量导入脚本、`npm run check:sgs-assets` harness。
- `check:props-boundary` 增加 `/game-assets` + `GAMEPROPS_ASSETS_DIR` 断言。
- 上传上限：原图 8MB、缩略图 1MB、边长 1–4096；MIME 仅 WebP/PNG/JPEG。

## 依据与验证状态

| 约束 | 值 | 来源 | 验证状态 |
|---|---|---|---|
| 批量素材规模 | 534 张 / 696 gid | `data/media/sgs/out/final-manifest.json`（本轮实测） | 已验证 |
| 卡面规格 | 512×720（缩略图 256×360） | 素材流水线交付规格 | 已验证 |
| `/game-assets` 缓存 | `public, max-age=31536000, immutable` | Express `immutable` 语义 | 已验证（`check:sgs-assets`） |
| 上传大小上限 | 原图 8MB / 缩略图 1MB | 内部约定（非外部约束） | 实现约束 |
