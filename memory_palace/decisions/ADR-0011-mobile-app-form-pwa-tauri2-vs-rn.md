# ADR-0011：移动端形态 —— PWA 优先、Tauri 2 壳次之，RN/Expo 延后

- **状态**：已采纳（2026-10-19）
- **相关**：ADR-0010（本 ADR 取代其「未来按 RN 复用」的前提措辞）、ADR-0003（web 公网可达）、`docs/modules/web.md`、`docs/modules/board-view.md`；不涉及 ADR-0009（主动消息门禁）
- **结论一句话**：现在不做 App（先 PWA）；要做壳就用 **Tauri 2**（Android 先行、壳加载线上页、不启用 IPC）；**RN/Expo 延后**到出现原生需求再重新决策

## 背景

1. **移动端从未立项**：仓库内无 RN / Expo / Tauri / Capacitor 工程；开发机（Windows）无 JDK、无 Android SDK、无 macOS（本机实测见末节）。
2. **ADR-0010 §3 把 RN 写成了既定前提**（「版图未来要被基于 RN 的 iOS/Android App 复用」），需要一次正式取舍，否则后续开工默认走 RN。
3. **需求边界很窄**：QQ 群推送短链 → 玩家手机点开 → 看公共资源 + 个人资源（版图 / 文本双模式、10s 轮询、**只读**）。所有写操作都在 QQ 群消息侧，App 不需要写入能力。
4. **前端不含 React**：`apps/web` 是 Express SSR + 20KB esbuild bundle，`src/view` + `src/client` 共 2232 行纯 TS；ADR-0010 §1 已明确「不引入 React/Vite」。
5. **鉴权形态天然适配壳方案**：token 只在 URL query + bootstrap JSON，8h TTL、`no-store`、刻意不落 localStorage（`apps/web/src/client/main.ts`）——壳直接加载「带 token 的线上 URL」即可，与现在点短链完全同构。
6. **跨端契约已就绪**：`BoardScene`（5 类图元、绝对坐标、无平台属性）+ `test-view-rn.mts` 守卫，服务端 SSR 与浏览器 bundle 共用同一份渲染函数。

## 决策

1. **短期（P0）：不建原生 App**。移动端交付物 = 现有 web 页 + PWA 化（manifest + 图标 + `display: standalone`）。不改鉴权、不改渲染、不改路由。
2. **需要「独立 App 图标 / 启动器」时（P1）：选 Tauri 2**，Android 先行；壳加载线上域名（`https://tabletopgame.20221219.xyz`），**不启用 IPC / Rust command**，使「改 web 即发布」（零发版更新）。**不采用 RN** 作为壳方案。
3. **RN / Expo 仅在下述任一条件成立时重新开 ADR**：
   - 需要**离开 App 的通知**（回合提醒）且不愿只依赖 QQ 群消息；
   - 版图演进为**原生级交互 / 动效**（3D 骰子、手势缩放、拖拽）；
   - 需要**相机 / 扫码 / 蓝牙 / NFC** 等原生硬件能力；
   - **iOS 上架**是硬需求且无 macOS 机器（此时 EAS 云构建是唯一通道）；
   - **web 端本身迁到 React**（届时 RN 复用成本骤降、天平反转）。
4. **`BoardScene` 契约保留不变**：IR 是「数据 → 场景」的宿主无关层，不因暂不做 RN 而废弃；文本态 + 版图态继续只在 web 侧维护一份实现。

## 理由（取舍）

### 复用率不对称（核心论据）

| 路径 | 需重写 | 量级 |
|---|---|---|
| **Tauri 2 壳** | 无业务代码（壳直接加载线上页 / 同一份 `view/**` 打包） | ≈ 0 行 |
| **RN 壳** | `svg.ts`(94) → react-native-svg 适配器、`text.ts`(269) 文本态、`page.ts`(140) 骨架、`client/main.ts`(182) 轮询/双模式/退避，外加 React + Metro/Expo 工程链与发版链 | 重写约 685 行 + 新框架 |
| RN 可直接复用 | `types.ts`(81) + `scene.ts`(791) + `theme.ts`(134) + `format.ts`(37) + `manifest.ts`(45) ≈ 1088 行（约占 `view/`+`client/` 的 49%） | — |

### 方案取舍

| 方案 | 结论 |
|---|---|
| 只做网页（现状） | 基线：零成本；但无 App 图标/启动器，且受 QQ 内置浏览器（X5）能力限制 |
| **PWA** | **采纳为先**：成本最低、改 web 即生效、无审核 |
| **Tauri 2 壳** | **采纳为 App 形态**：复用 ~100%，Android 可在 Windows 上完整开发；iOS 需 macOS（阻塞项，见「影响」） |
| RN / Expo | **当前否**：引入 React + 双渲染维护；唯一确定优势是「无 Mac 也能 EAS 云构建 iOS」 |
| Capacitor | **备选**：Tauri 移动端插件生态不足时的同类替代（同为 web-first 壳）；iOS 本地构建同样需 macOS，但 CI/EAS 生态更成熟 |

### 风险与缓解

| 风险 | 缓解 |
|---|---|
| 国产 Android 系统 WebView 碎片化（无 Play 长期不更新） | bundle target 由 `es2020` 降到 `es2017`；Tauri `minSdkVersion` 提到 24+（官方最低即 Android 7.0/SDK 24）；上线前用低端国产 ROM 真机目检版图 |
| 纯 WebView 壳上架 App Store 有 §4.2「最低功能」拒审风险 | 若要上架必须补原生能力（通知 / 分享 / 深链），「零成本壳」优势打折；若仅小圈子分发，走 APK 侧载（QQ 群文件 / 链接）绕开商店与审核 |
| App 内无法天然携带 token | 壳方案直接吃「带 token 的 URL」，与点短链同构；将来若要「App 内直接进」，再评估 deep link 插件 + server 一次性 token |

## 影响

- **ADR-0010 §3 的「基于 RN 的 iOS/Android App」措辞以本 ADR 为准**。不修改 ADR-0010 正文（避免重写历史，按时间序并读）。
- `docs/modules/board-view.md` 的「RN 映射表 / RN 契约守卫」按**通用跨宿主映射**保留，不因本决策删除；若 Tauri 壳落地，再补一节「WebView 宿主约束」。
- **本期无代码改动**（仅决策落盘）。后续立项待办（未排期）：
  - PWA：manifest + 图标 + `display: standalone`，核对 `/state` 与页面 `no-store` 在 PWA 下的表现；
  - `build:client` 的 target 评估（`es2020` → `es2017`）；
  - Android 开发环境（JDK + Android SDK + NDK）安装 —— 仅 P1 立项时才需要；
  - iOS 需求与上架意愿的确认（决定 Tauri 是否出局、是否转 Expo/EAS）。
- 新增依赖面：P0 无；P1 仅 Tauri CLI + Android 工具链（Rust 侧已具备，本机 `rustc` 已装）。

## 依据与验证状态

| 事实 | 来源 | 验证状态 |
|---|---|---|
| Tauri 2 的 iOS 开发需要 Xcode，且**仅 macOS 可用**（原文：「iOS development requires Xcode and is only available on macOS.」） | <https://v2.tauri.app/start/prerequisites/> | 已验证（2026-10-19 抓取原文） |
| Tauri 2 最低支持 Android 7.0 / SDK 24（原文：「The minimum supported Android version for Tauri apps is Android 7.0 (codename Nougat, SDK 24).」） | <https://v2.tauri.app/distribute/google-play/> | 已验证（2026-10-19 抓取原文） |
| EAS Build 的 iOS 构建跑在 Expo 托管的 macOS 云端（原文：「iOS builds run on macOS runners hosted in Expo's macOS cloud.」）→ 无 Mac 也能出 iOS 包 | <https://docs.expo.dev/build/introduction/> | 已验证（2026-10-19 抓取原文） |
| App Store 审核 §4.2「最低功能」可能拒纯网页壳 | Apple App Store Review Guidelines §4.2 Minimum Functionality | **待复核**（本机抓取被重置，按公开常识记录；上架前必须复核原文） |
| 本机环境：`rustc`/`cargo` 1.99.0 已装；`java` / `adb` / `ANDROID_HOME` / `ANDROID_SDK_ROOT` 均不存在；无 macOS | 本机实测（`rustc --version`、`java -version`、`adb version`、环境变量） | 已验证（2026-10-19） |
| 产物体积量级（Tauri 壳数 MB / RN 二十 MB 量级） | 工程通识 | **未实测**，仅作参考，**不作为决策依据** |
