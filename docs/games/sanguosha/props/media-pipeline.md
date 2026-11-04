# 三国杀武将素材流水线（剧照采集 → 卡面生成）

> 状态：**采集 + 卡面生成已完成（2026-11-03）**。`out/final/` 交付 **534 张** 512×720 统一画风卡面（534 个武将桶全覆盖）。
> 产物**不写库、不上传**（入中台见 §16，另立任务）。
> 代码：采集 `apps/scripts/sgs-media/`；生成 `data/media/sgs/p1/`；资源闸门 `apps/scripts/media-gate.mjs`。

## 1. 目标与口径

两步走：① 从 **1994 央视《三国演义》**（CCTV 版，480×360 / 25fps / 每集约 43 分钟）自动截取**人物介绍字幕卡**画面（写实素材 + 风格/身份参考）；② 以它为参考合成每位武将的**赛璐璐（漫画）卡面**。

- 字幕卡形态（实测确认）：**竖排两列**——左列姓名（如 `何进`），右列 `「字 X」`（如 `「字 遂高」`）；**左右两侧都会出现**；纵向大致在画面 16%–72% 区间。
- 采集口径：**宁漏不误**。匹配只做「归一化后精确 / 完整包含（冗余 ≤2 字）」，不做编辑距离容错；`置信度 ≥ 0.5`。
- 覆盖口径：696 位武将去装饰前缀（`SP/界/谋/神/魔/族`）后归并为 **534 个桶**；找不到素材的桶留空。素材**不设上限**，同镜头去重后全量保留。
- 片头/片尾本轮**不过滤**（先全量抓，后续按 `hits.jsonl` 的 `orientation/text_height/bg_motion` 特征再定规则）。

## 2. 目录约定（C 盘只放模型，其余全在 D 盘仓库内）

| 内容 | 路径 |
|---|---|
| OCR 模型权重 | `C:/llm/models/rapidocr/*.onnx`（`SGS_RAPIDOCR_DIR` 可覆盖） |
| Python venv | `data/media/sgs/venv`（独立，不污染 `C:/llm/comfy-venv`） |
| 分片缓存 | `data/media/sgs/src/<guid>/`（处理完即删） |
| 命中帧 | `data/media/sgs/hits/<ep>/<桶>_<mmss>_<L|R>_<seg>.png`（原分辨率） |
| 报告 | `data/media/sgs/out/<ep>/{hits,texts,unmatched}.jsonl`、`stats.json`、`report.md`、`contact-sheet.jpg` |
| 全季汇总 | `data/media/sgs/out/batch-summary.json`、`batch.log` |

`data/` 已被 `.gitignore` 全量忽略，素材不会进仓库。

## 3. 资源独占闸门（用户硬要求）

采集必须在**项目服务全部停止、资源释放**时进行：

```bash
npm run dev:down                       # 停整条开发链 + redis
node apps/scripts/media-gate.mjs       # 九项检查
node apps/scripts/media-gate.mjs --acquire   # 通过后写 data/run/media.lock
# …跑采集…
node apps/scripts/media-gate.mjs --release   # 释放锁
```

检查项：`dev-up` 进程、`server.exe/llama-server.exe/esbuild.exe`、端口 `3000/18080/5174/6379/8188/4002`、Redis 容器、显存（≤4500 MiB，本机基线 ≈2.5GB）、可用内存（≥8GB）、目标盘（≥50GB）、独占锁。

## 4. 用法

```bash
cd apps/scripts/sgs-media
PY=data/media/sgs/venv/Scripts/python.exe   # 相对仓库根
P="D:/code/tabletop_game/$PY"

$P -m sgs_media dict                       # 打印桶/词典统计（534 桶 / 618 词条）
$P -m sgs_media probe  --page <页面URL>     # 探测 guid/时长/分辨率/码率
$P -m sgs_media frames --page <URL> --at 62,428   # 导出指定时刻整帧（调 ROI 用）
$P -m sgs_media episode --page <URL> --label ep01 --mode side --fps 1
$P -m sgs_media batch  --from 1 --to 84 --workers 6 --fps 1   # 全季并行 + 断点续跑
$P -m sgs_media report --label ep01        # 重建报告
```

`--mode side`（默认）：左右侧栏人物卡；`--mode bottom`：底部台词字幕（按台词里出现的武将名抓「出场镜头」，另一条备选路线）。

## 5. 实现要点（踩过的坑）

1. **取流**：页面 → `guid` → `vdn.apps.cntv.cn/api/getHttpVideoInfo.do?pid=<guid>` → HLS 变体 `/asp/hls/{450|1200|2000}/…/{br}.m3u8`（VOD，10–15s 分片）。实测 2000 档仍是 **480×360**。无需登录、无需 ffmpeg（PyAV 直接解 `.ts`）。
2. **时间轴**：CCTV 的 `.ts` 分片 **PTS 是整片绝对值**，不能再叠加段起点（早期版本叠加导致所有时间码错位、dump 帧全错）。
3. **竖排**：人物卡是竖排，横排检测器（要求宽 > 高×1.4）100% 漏。正确做法：**把侧栏 ROI 交给 PP-OCR 的 det**（它按框方向旋转裁剪）；**不能整体旋转 ROI** 再送 rec（字形侧躺，识别全废）。
4. **对比度**：老片侧栏字幕偏灰，直接 det 会漏（`何进` 复现）；侧栏 OCR 前加 **CLAHE(clip=2.5)** 后稳定命中。
5. **固定叠加物遮罩**：左上 `CCTV8`+`电视剧` 台标需遮（`OVERLAY_RECTS`），否则每帧误检。
6. **采样**：字幕停留 >2s，**1 fps** 足够；去重规则 = 同桶 + 同侧 + 同横竖 + 时间窗 1.5s 内（文本相同或 dHash 汉明 ≤5）→ 留最清晰一帧。
7. **OCR 档位**：`det_limit_side_len=480`（384 会漏 `何进`，736 反而更差）；底部字幕模式用「行投影」定位 + 单行 rec-only。

## 6. 实测性能（Intel Ultra 5 245K / RTX 5070 Ti 闲置）

| 项 | 数值 |
|---|---|
| 单集（43 分钟，1fps） | **≈ 7–8 分钟**（单进程） |
| 3 集 / 3 并发 | 7.8 分钟墙钟 |
| 84 集 / 6 并发 | **≈ 2 小时**（预估，按 7.5 min/集 ÷ 6） |
| 单集 OCR 调用 | ≈ 3000 次（gate 过滤后） |
| 解码 | 65k 帧 / 6–7 s（软解 9,664 fps，**不需要 GPU 硬解**） |

## 7. 产物字段（`hits.jsonl`）

`t`（全片秒）、`side`（L/R/B）、`orientation`（h/v）、`text`（OCR 原文）、`bucket`（武将桶）、`kind`（exact/contains/subtitle）、`conf`、`box`（整帧坐标 x,y,w,h）、`text_height`、`bg_motion`、`sharp`（拉普拉斯方差）、`dhash`、`file`、`seg`、`names`（一句台词命中的多个桶）。

## 8. 已知限制 / 待办

- 召回仍非 100%：个别卡只认出「字」列（如某集 62.4s 的曹操卡）。可上 2fps 或「两列合并再 OCR」提升，代价翻倍。
- `--mode bottom` 走「台词提到武将名」路线，OCR 会把名字识错（`刘焉`→`刘离`），按「宁漏不误」暂不加容错。
- 片头/片尾未过滤（剧名卡、演员表可能命中同名）。
- 素材尚未接入中台（见 §16）。

---

# 第二部分：卡面生成（剧照 → 赛璐璐卡面）

## 9. 总览与产出

534 个桶 = 696 位武将按 `SP/界/谋/神/魔/族` 前缀去重组并；每桶出 2 个候选，人工筛定 1 张。

| 来源 | 桶数 | 做法 | 候选目录 |
|---|---|---|---|
| A. **有剧照** | 93 | Animagine XL 3.1 + **IPAdapter 参考剧照**（保演员/服饰特征） | `out/gen5/` |
| B. **空缺** | 441 | Animagine XL 3.1 + Qwen 外观描述 + 逐桶哈希美术指导（纯 prompt） | `out/gen4/` |
| **收口** | **534** | 统一命名 `<势力>_<桶名>.png`，512×720 | **`out/final/`** |

清单：`out/final-manifest.json`（桶 ↔ 文件 ↔ 来源图 ↔ 势力/体力/包/全部 gid ↔ 候选数）。

## 10. 模型与环境

| 件 | 位置 | 说明 |
|---|---|---|
| ComfyUI | `C:/llm/ComfyUI`（venv `C:/llm/comfy-venv`，端口 8188） | 出图宿主 |
| **Animagine XL 3.1** | `models/checkpoints/animagine-xl-3.1.safetensors`（6.94GB） | **定稿主模型**（SDXL 动漫底模，吃 Danbooru 标签） |
| SDXL base 1.0 | 同上 | 早期使用，**已弃用**（同质化严重） |
| IPAdapter Plus (SDXL) | `models/ipadapter/ip-adapter-plus_sdxl_vit-h.safetensors` | 剧照参考 |
| CLIP-ViT-H-14 | `models/clip_vision/CLIP-ViT-H-...-**.bin**` | **必须保留 `.bin` 后缀**（改成 `.safetensors` 会报 header too large） |
| 自定义节点 | `custom_nodes/ComfyUI_IPAdapter_plus` | `git clone` |
| Qwen2.5-3B-Instruct Q4 | `C:/llm/models/qwen2.5-3b-instruct-q4_k_m.gguf` + `llama-server --port 8090` | 批量写外观描述（441 条约 2.4 分钟） |
| 下载源 | **ModelScope**（实测 10–45 MB/s） | `hf-mirror` 仅 ≈0.4 MB/s，只作兜底 |

## 11. 阶段一：有剧照的 93 桶（`p1/gen5_refs_animagine.py`）

流程：命中帧 → 裁剪 → IPAdapter 参考（w=0.45）→ Animagine 出图 → 512×720。

**裁剪规则（关键）**：参考图必须去掉字幕/台标/水印，否则 IPAdapter 会把它们一起画进成品（实测出现过 bilibili 水印与竖排字幕被复刻）。
- 人物卡在左半区：取 `x 34%–94%`；在右半区：取 `x 6%–66%`；纵向统一 `y 20%–78%`（避开顶台标、底部台词条、右上水印）。
- 再居中方裁到 1024×1024 喂 IPAdapter。

**特征强化（必须，否则跑偏）**：

| 武将 | 加权正向词 | 反向词 |
|---|---|---|
| 关羽 | `(crimson red face:1.45)`、`(magnificent long flowing black beard:1.35)` | `old man, white beard` |
| 张飞 | `(savage roaring warrior:1.3)`、`(wild bristling jet-black beard:1.5)` | `old man, white beard, grey hair, calm expression` |
| 曹操 | `(cunning ambitious warlord, ruthless piercing gaze:1.4)` | 见 §13 年龄标签 |
| 赵云 | `white-armored cavalry commander, white plume, white warhorse` | — |
| 周瑜 | `(refined handsome young commander:1.3)` | — |

**备选工艺：剧照直描 img2img（`p1/restyle.py`）**：想把原剧照"描"成漫画（姿势/表情逐像素继承）时用；**denoise 拐点 = 0.55**（0.3/0.4 只是滤镜，出不来线稿）。

## 12. 阶段二：空缺的 441 桶（`p1/gen_from_desc3.py`）

1. **Qwen2.5-3B 逐将写外观描述**（`p1/gen_prompts_qwen.py` → `out/p2/desc_final.jsonl`）：输入名称/势力/体力/技能名与描述/生平，输出 ≤55 词的英文外观短语；清洗规则＝去 `signature motif`（会画出兽耳犄角）、去 `age NN`（与标签冲突）、女性去胡须词、去残留中文。
2. **标签体系**（Danbooru 风格，`tags(bucket)`）：年龄组（ELDERLY / MIDDLE / YOUNG / FEMALE，白名单判定）+ 发型 + 须型 + 眼型/脸型。
3. **美术指导随机化**：构图 / 机位 / 背景 / 光线 / 表情 / 氛围元素 / 风格子变体，全部按 `md5(桶名)` **确定性抽样**——同一武将重跑一致，不同武将必然不同。
4. 采样：`euler_ancestral / 28 步 / cfg 6.0 / 832×1216 → 512×720`。

**踩坑教训**：
- 纯 SDXL base + 模板描述 → **1323 张里只有 66 张不同**（ComfyUI 执行缓存 + 描述同构）；
- 换 Animagine 后姿态/背景/光影才真正拉开；
- **年龄必须显式打标签**：不打标签时满屏年轻帅哥；曹操曾因被归入"老者"变成白发老人（已改为 `MIDDLE`）。

## 13. 参数结论（可复用）

| 结论 | 值 |
|---|---|
| img2img 照片→漫画拐点 | **denoise 0.55**（0.45 及以下只是滤镜） |
| IPAdapter 权重（剧照参考） | **0.45**（>0.55 拉回写实，<0.3 丢失身份/红脸/须形） |
| 出图尺寸 | 832×1216 出 → LANCZOS 降到 **512×720**（512 直出掉质量） |
| 必备反向词 | `photorealistic, photo, 3d, semi-realistic, caucasian, western face, blonde hair, blue eyes, watermark, text, chinese calligraphy, multiple people` |
| 东方脸约束 | 正向 `chinese, east asian, black hair`（不加会变欧美脸） |
| 一致性三件套 | 同一 checkpoint + 同一质量标签（`masterpiece, best quality, very aesthetic`）+ 逐桶确定性抽样 |
| 性能 | Animagine 832×1216 ≈ 8–10s/张；882 张 ≈ 90 分钟（RTX 5070 Ti，单进程） |

## 14. 交付规格

- `out/final/*.png` —— **534 张，512×720，`<势力>_<桶名>.png`**
- `out/final-manifest.json` —— 每张记录 `bucket / file / src（候选来源）/ size / kingdom / hp / pack / gids（该桶对应全部武将 gid）/ names / candidates`
- 换图流程：把新图覆盖 `out/final/` 同名文件，或改 `gen4|gen5` 里的候选后重跑组装脚本即可

## 15. 复现命令

```bash
# 1) 起 Qwen（写描述用）
C:/llm/llama.cpp-master/build/bin/Release/llama-server.exe -m C:/llm/models/qwen2.5-3b-instruct-q4_k_m.gguf --port 8090 -ngl 99 -c 4096 --no-webui
# 2) 起 ComfyUI
C:/llm/comfy-venv/Scripts/python.exe main.py --listen 127.0.0.1 --port 8188   # 工作目录 C:/llm/ComfyUI
# 3) 写描述（441 空缺桶 / 93 剧照桶）
$PY data/media/sgs/p1/gen_prompts_qwen.py --out desc.jsonl
$PY data/media/sgs/p1/gen_prompts_qwen.py --include-covered --only "$(cat data/media/sgs/out/hit_buckets.txt)" --out desc93.jsonl
# 4) 出图
$PY data/media/sgs/p1/gen_from_desc3.py --seeds 31,62      # 441 空缺桶（gen4）
$PY data/media/sgs/p1/gen5_refs_animagine.py --seeds 31,62 # 93 剧照桶（gen5）
```

## 16. 后续（素材入库已完成；其余另立任务）

- **入中台（已完成，2026-11-04，ADR-0036）**：`prop_assets` 内容寻址上传 + 缩略图/裁剪、`image_asset_id` 绑定、Web 对局页展示。
  - 批量导入：`node apps/scripts/import-sgs-general-assets.mjs`（先 `--dry-run`）→ WebP(512×720) + 缩略图(256×360) → 按 manifest `gids` 绑定 696 个 `sgs_hero_<gid>`；报告 `data/media/sgs/out/import-report.json`。
  - 中台手工：`/props-admin/assets` 上传/裁剪；`/generals` 等页面「选择素材 / 上传并绑定」。
  - 素材目录：`data/gameprops/assets/`（gitignore）；web 同源 `/game-assets`（immutable）；门禁 `npm run check:sgs-assets`。
  - 版权：仍为**内部开发使用**，上线前确认授权或替换。
- 版权/肖像：素材源于 1994 版电视剧 + 生成图，**上线前需确认授权或替换为自绘**；`out/final` 仅作内部开发使用。
