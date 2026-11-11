# 红外照片分析工具（IR Inspection Tool）

配网无人机（大疆御 3T / Mavic 3T）与手持 FLIR 红外照片的**纯本地离线分析工具**：辐射原图逐像素测温、伪彩画布、ROI 统计定级、温度极值标注、批量解析与离线 GPS 点位地图、SQLite 巡检台账、Word 缺陷报告导出。

> 技术栈：Tauri 1.x + Rust（`thermal` crate 解析 FLIR FFF 段 R-JPEG、大疆 XMP/EXIF 适配、`kamadak-exif` 读取 GPS、`image` 生成标注 PNG / 缩略图）+ Vue 3 + Fabric.js + Element Plus + rusqlite + docx-rs。
> 目标平台：统信 UOS20（Debian10）内网离线环境；开发期可在 Windows（WebView2）运行。

## 功能清单（对照开发文档 F1–F16）

| 编号 | 功能 | 入口 |
| --- | --- | --- |
| F1 | 拖拽/选择导入御 3T / FLIR 辐射原图 | 分析页「导入辐射原图」或直接拖入画布 |
| F2 | 伪彩渲染 + 滚轮缩放到像素级 + 拖拽平移 | 画布（滚轮 / 平移工具 / 空格拖拽） |
| F3 | 鼠标实时取温（十字线 + 温度提示） | 画布上移动鼠标 |
| F4 | 调色板（铁红/彩虹/灰度/白热）+ 色标上下限 | 右栏「调色板」 |
| F5 | ROI 矩形统计 max/min/avg/ΔT + 四级判定 | 画布工具「矩形」 |
| F6 | 元数据（机型/时间/GPS/ε/距离/湿度/反射温） | 右栏「元数据」 |
| F7 | 发射率/距离修正，温度矩阵即时重算 | 顶栏「发射率 / 距离」输入框 |
| F8 | 导出温度矩阵 CSV / 带 ROI 与温度极值标注 PNG | 顶栏「导出 CSV / 导出标注图」 |
| F9 | UOS20 deb 离线安装包（GitHub Actions 交付构建） | 见「交付构建」章节与 [`packaging/README.md`](packaging/README.md) |
| F10 | 多边形 / 线段测温；ROI 形状可拖动、缩放、多选 | 画布工具「多边形 / 线段」（双击或回车结束） |
| F11 | 三相同部位 ΔT 比对告警 | ROI 面板给 ROI 标注相别（≥2 相自动比对） |
| F12 | 缺陷阈值可配置（接头温度/温差/相间） | 顶栏「阈值设置」 |
| F14 | 批量文件夹遍历解析 + 进度 + 结果导出；解析结果按 GPS 展示在纯离线点位地图，可查看缩略图并跳转分析 | 「批量处理」页「表格 / 地图」视图 |
| F15 | SQLite 巡检台账（增删改查/筛选/统计） | 「巡检台账」页 |
| F16 | 国网风格 Word 缺陷报告 | 台账页勾选记录 →「生成缺陷报告」 |

> F13（MSX 融合）与 F17（AI 识别）按计划留待后续；F9 的交付构建流水线已就绪，目标机安装验收待补。
>
> 画布工具栏「温度标注」可一键显隐全图与当前选中 ROI 的最高温/最低温（最多 4 处），F8 导出标注图与画布所见一致；离线点位地图为纯 SVG 绘制，无底图、无外部瓦片服务，点位缩略图按需懒加载（仅 JPG、单文件 ≤64MB）。

## 前置条件：必须是辐射原图

工具只对**辐射 R-JPEG** 有效（内嵌 16bit 热数据与标定参数）：

- **御 3T（DJI Fly）**：拍照前进入红外相机设置，开启「**保存原图 / 辐射点温度数据**」，文件名以 `_T.JPG` 结尾（如 `DJI_20260804234634_0002_T.JPG`）。普通导出的调色预览图没有热数据，导入会明确报错。
- **FLIR**：使用相机原始 R-JPEG（含 FFF 段）。

## 开发环境运行

推荐用仓库自带的本机构建入口（自动把 `~/.cargo/bin` 加入 PATH，并在 pnpm 缺失时回退 corepack）：

> 首次使用请先安装与 `Cargo.lock` 中 tauri crate（1.8.3）同版本的 tauri CLI：
> `cargo install tauri-cli --version 1.8.3 --locked`
> （npm 上的 `@tauri-apps/cli` 1.x 只发布到 1.6.3，与 1.8.3 主次版本不一致会被 CLI 拒绝执行）

```bash
node scripts/build-local.mjs env      # 环境自检
node scripts/build-local.mjs install  # 安装前端依赖
node scripts/build-local.mjs dev      # 启动 Tauri 开发模式
node scripts/build-local.mjs build    # 本机调试包（不可交付 UOS20）
node scripts/build-local.mjs test     # 跑全部自动化验收
```

已装好 pnpm 的环境也可直接用原生命令：

```bash
pnpm install
pnpm tauri dev     # Windows 需 WebView2 运行时（Win10/11 一般自带）
```

对应关系：`pnpm dev:local` / `pnpm build:local` / `pnpm build:env` 分别等价于上面的 `dev` / `build` / `env` 子命令。

- Windows 构建注意：本仓库默认 GNU 工具链，且 Tauri CLI 不识别 `CARGO_TARGET_DIR`；
  若工作区路径含特殊字符导致 windres/ld 失败，请设置 `CARGO_TARGET_DIR` 指向纯 ASCII
  路径（仅影响 `cargo` 直调，`tauri dev` 场景见下）。
- Node ≥ 20.19（或 ≥ 22.12，Vite 8 的 engines 要求），Rust ≥ 1.88（`image 0.25` 的 MSRV）。

### 用真实样本自验（M1）

```powershell
cd src-tauri
$env:CARGO_TARGET_DIR = "D:\Rust\ir-target"   # 任意纯 ASCII 路径
cargo run --bin dump_temps -- ..\DJI_20260804234634_0002_T.JPG ..\e2e\sample.json
```

预期输出 `written ..\e2e\sample.json (640x512)`，JSON 内含温度矩阵与元数据
（御 3T：640×512，ε=0.95，距离 5 m）。

### 前端 mock 端到端验收（无需真机窗口）

```powershell
pnpm dev                                    # 终端 1
node scripts/e2e-cdp.mjs                    # 终端 2（headless Chrome + mock IPC）
```

验收项：加载 IPC → 伪彩铺满渲染 → 悬停取温 → ROI 绘制 → 后端统计回填 → 定级展示。

## 交付构建（GitHub Actions）

UOS20 内网机的安装包由 **GitHub Actions** 在 Debian 10 容器中构建，产物下载后拷入内网即可安装：

- 流水线：`.github/workflows/build-uos20.yml`（`debian:buster` 容器，glibc 2.28，与 UOS20 同源）
- 触发：Actions 页手动 `Run workflow`；push 到 `main` 或提 PR 做构建校验；推送 `v*` 标签额外发布 GitHub Release
- 产物：`ir-inspection-tool_0.1.0_amd64.deb`（必需）、`ir-inspection-tool_0.1.0_amd64.AppImage`（可选）、`SHA256SUMS`
- 工具链：Node 20.19+、`pnpm@10.24.0`、Rust 1.90、**cargo 版 tauri CLI 1.8.3**（npm 上 `@tauri-apps/cli` 的 1.x 止于 1.6.3，与 `Cargo.lock` 的 tauri crate 1.8.3 主次版本不匹配会被 CLI 拒绝）
- 校验：流水线断言产物依赖的最高 GLIBC 符号版本不超过 2.28，超出即判定构建失败

**本机构建不产出交付包**：开发机 glibc 高于 UOS20（如 Deepin 25 为 2.38），AppImage 同样依赖宿主 glibc。完整说明与内网安装步骤见 [`packaging/README.md`](packaging/README.md)。

## 数据落盘位置

- 巡检台账：`~/.local/share/ir-inspection-tool/ledger.db`（Windows：`%APPDATA%`）
- 阈值配置：应用配置目录 `thresholds.json`

## 目录结构

```
├── src/                    # Vue3 前端
│   ├── api/tauri.js        # 21 个 Tauri IPC 命令封装
│   ├── stores/             # pinia（6 个）：image/roi/batch/ledger/settings/view
│   ├── utils/              # palettes（调色板 LUT）/ defects / format /
│   │                       # mapProjection（GPS 投影）/ roiShape（形状换算）/ tempMarks（极值标注）
│   ├── components/         # AnalysisView/CanvasView/ColorScale/RoiPanel/MetaPanel/
│   │                       # BatchPanel/MapView/LedgerPanel/ReportDialog/SettingsDialog
│   └── App.vue             # 图像分析 / 批量处理 / 巡检台账 三视图
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── main.rs         # 命令注册（21 个）+ 配置初始化
│   │   ├── lib.rs          # 模块导出（主 crate 与 backend-tests 共用）
│   │   ├── commands.rs     # Tauri command 实现
│   │   ├── thermal.rs      # R-JPEG 解析 + EXIF/XMP（thermal crate + 大疆适配）
│   │   ├── dji.rs          # 御 3T 辐射数据段适配
│   │   ├── temperature.rs  # 普朗克换算（ε/距离 serde 往返修正）
│   │   ├── roi.rs          # ROI 掩码统计（矩形/多边形/线段 + 极值坐标）
│   │   ├── diagnosis.rs    # 四级判定 + 三相 ΔT（F11/F12）
│   │   ├── export.rs       # CSV / 标注 PNG（含温度极值标注，F8）
│   │   ├── thumbnail.rs    # 红外原图缩略图 data URL（F14 点位弹窗）
│   │   ├── db.rs           # SQLite 台账（F15）
│   │   ├── report.rs       # docx 缺陷报告（F16）
│   │   ├── batch.rs        # 文件夹批量解析（F14）
│   │   ├── settings.rs     # 阈值持久化（F12）
│   │   ├── utils.rs        # base64 / 等级名等通用函数
│   │   ├── error.rs        # 错误类型
│   │   └── bin/dump_temps.rs  # 真实样本温度矩阵导出
│   ├── backend-tests/      # 无 GTK 依赖的独立测试 crate
│   ├── Cargo.toml / Cargo.lock
│   └── tauri.conf.json     # UOS20 打包配置
├── scripts/                # run-tests.mjs（统一测试入口）/ build-local.mjs（本机构建入口）/
│                           # check-e2e.mjs（IPC 契约核查）/ verify-sample.mjs（样本基线）/
│                           # e2e-cdp.mjs（mock 验收）等
├── tests/                  # 6 个前端逻辑测试文件
├── packaging/README.md     # 交付构建与内网安装指引
├── .github/workflows/      # build-uos20.yml（UOS20 交付构建流水线）
└── e2e/                    # sample.json（真实样本温度矩阵）/ mock-app.png
```

## 测试与验收

一条命令跑通全部自动化验收（测试入口零新增依赖；交付构建由 GitHub Actions 流水线完成，本机只做开发与调试构建）：

```bash
pnpm test                 # 全部通道
pnpm test:rust            # Rust 单测 + 真实样本全链路
pnpm test:frontend        # 前端纯逻辑
pnpm test:sample          # 样本基线
pnpm test:e2e             # E2E 现状核查
```

| 通道 | 实际命令 | 覆盖 |
| --- | --- | --- |
| Rust | `cargo test --offline`（`src-tauri/backend-tests`） | 50 项：R-JPEG 解析 / 测温 / ROI / 定级 / 台账 / 导出 / 批量 / 阈值 / 缩略图 / 极值标注，含两张真实辐射原图 |
| 前端 | `node --test tests/*.test.js` | 45 项（6 个测试文件）：调色板 LUT、缺陷等级、格式化、21 个 IPC 命令契约、6 个 pinia store，以及 GPS 投影 / ROI 形状 / 温度极值标注 / 视图跳转 |
| 样本基线 | `node scripts/verify-sample.mjs` | `e2e/sample.json` 的 15 字段、640×512 矩阵、ε / 距离 / 温度量程 |
| E2E | `node scripts/check-e2e.mjs` | 静态核对 21 个注册命令 / 21 个前端封装 / 9 个 mock 分支 |

### 为什么后端测试放在 `src-tauri/backend-tests/`

主 crate 的任意 cargo target 都会链接 tauri → `webkit2gtk-4.0` / `gtk-3` / `glib-2.0` 开发库；精简开发机与内网 CI 通常只有运行时库（早期开发机仅有 4.1），`gdk-sys` / `gio-sys` 的 build script 会直接失败。独立 crate 沿用 `bin/dump_temps.rs` 的 `#[path]` 做法，复用**同一份**源码模块与内置 `#[cfg(test)]` 用例，不链接 tauri，因此完全离线、无 GTK 依赖。开发环境若已装齐 GTK 4.0 开发库，仍可额外执行 `cd src-tauri && cargo test --lib` 验证完整 lib 目标。

### 工具链与样本

- 入口脚本自动把 `~/.cargo/bin` 加入 PATH；Node < 18 时自动从 NVM 目录（`NVM_DIR`，默认 `~/.config/nvm`）挑选最高版本自举。
- 真实样本经 `IR_SAMPLE_JPG` / `IR_SAMPLE_DJI` 注入（也写在 `backend-tests/.cargo/config.toml`）；样本缺失时测试**直接失败**，不静默跳过。
- 测试临时目录均带进程号与时间戳，支持 `cargo test` 并行与重复执行。

### 验收记录（2026-11-04，历史基线）

> 以下为当时的实测结论，保留原样；当前代码基线的复核结果见下节。

- Rust 32/32、前端 20/20、样本基线、E2E 静态核查全部通过。
- 真实样本 `DJI_20260804234634_0002_T.JPG`：640×512、ε=0.95、距离 5 m、28.96~32.70 ℃；CSV / 标注 PNG / Word 报告产物均可生成并解码。
- `scripts/e2e-cdp.mjs` 因本机无 Chrome/Chromium 且默认路径为 Windows，CDP 图形验收记为**环境受限项**；在有浏览器的机器设置 `CHROME_PATH` 并先启动 `pnpm dev` 后可复现。

### 当前基线复核（2026-11-11）

- Rust 50/50、前端 45/45、样本基线校验、E2E 静态核查（21 个注册命令 / 21 个前端封装 / 9 个 mock 分支）通过。
- 新增用例覆盖：离线 GPS 投影与坐标过滤、ROI 形状坐标换算、温度极值标注、缩略图（扩展名 / 文件大小 / 长边钳制 / 真实 R-JPEG 可解码）、带温度标注的 PNG 导出、全图极值缓存与发射率修正后重取、视图 store 跳转分析。
- `scripts/e2e-cdp.mjs` 的 CDP 图形验收仍为环境受限项（本机无 Chrome/Chromium），设置 `CHROME_PATH` 并先启动 `pnpm dev` 后可在有浏览器的机器复现。

## 已知事项

- Windows-GNU 下 `cargo test` 的测试二进制存在 WebView2Loader 符号解析问题
  （应用本体与 Linux 目标不受影响）；单元测试在 Linux/CI 上运行，现统一走无 GTK 依赖的 `backend-tests`。
- headless 验收脚本中 Vue 异步 flush 存在延迟，脚本已用强制刷新兜底。
- 离线点位地图为纯 SVG 绘制，无底图、无外网瓦片；仅展示带有效 GPS 的点位，无坐标项单独计数提示。
- 点位弹窗缩略图按需懒加载并缓存，仅支持 `.jpg` / `.jpeg`、单文件 ≤64MB、长边默认 320px（钳制 64~1024），失败时降级为占位图。
- 批量解析为顺序执行（100 张内无感），后续可换 rayon 并行。
- 交付给 UOS20 的安装包必须来自 GitHub Actions 流水线（Debian 10 环境）；本机直接构建的 deb / AppImage 因 glibc 过高无法安装到目标机。
- AppImage 为可选产物：linuxdeploy 在构建容器内不可用时不会产出，deb 为必需产物。
- 交付构建过程需要联网拉取依赖（GitHub Actions 环境），但产物本身不含任何运行时外网依赖，安装后完全离线可用。
