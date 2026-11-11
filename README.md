# 红外照片分析工具（IR Inspection Tool）

配网无人机（大疆御 3T / Mavic 3T）与手持 FLIR 红外照片的**纯本地离线分析工具**：辐射原图逐像素测温、伪彩画布、ROI 统计定级、批量解析、SQLite 巡检台账、Word 缺陷报告导出。

> 技术栈：Tauri 1.x + Rust（`thermal` crate 解析 FLIR FFF 段 R-JPEG）+ Vue 3 + Fabric.js + Element Plus + rusqlite + docx-rs。
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
| F8 | 导出温度矩阵 CSV / 带 ROI 标注 PNG | 顶栏「导出 CSV / 导出标注图」 |
| F10 | 多边形 / 线段测温 | 画布工具「多边形 / 线段」（双击或回车结束） |
| F11 | 三相同部位 ΔT 比对告警 | ROI 面板给 ROI 标注相别（≥2 相自动比对） |
| F12 | 缺陷阈值可配置（接头温度/温差/相间） | 顶栏「阈值设置」 |
| F14 | 批量文件夹遍历解析 + 进度 + 结果导出；解析结果按 GPS 展示在纯离线点位地图，可查看缩略图并跳转分析 | 「批量处理」页「地图」视图 |
| F15 | SQLite 巡检台账（增删改查/筛选/统计） | 「巡检台账」页 |
| F16 | 国网风格 Word 缺陷报告 | 台账页勾选记录 →「生成缺陷报告」 |

> F13（MSX 融合）与 F17（AI 识别）按计划留待后续。

## 前置条件：必须是辐射原图

工具只对**辐射 R-JPEG** 有效（内嵌 16bit 热数据与标定参数）：

- **御 3T（DJI Fly）**：拍照前进入红外相机设置，开启「**保存原图 / 辐射点温度数据**」，文件名以 `_T.JPG` 结尾（如 `DJI_20260804234634_0002_T.JPG`）。普通导出的调色预览图没有热数据，导入会明确报错。
- **FLIR**：使用相机原始 R-JPEG（含 FFF 段）。

## 开发环境运行

```powershell
pnpm install
pnpm tauri dev     # Windows 需 WebView2 运行时（Win10/11 一般自带）
```

- Windows 构建注意：本仓库默认 GNU 工具链，且 Tauri CLI 不识别 `CARGO_TARGET_DIR`；
  若工作区路径含特殊字符导致 windres/ld 失败，请设置 `CARGO_TARGET_DIR` 指向纯 ASCII
  路径（仅影响 `cargo` 直调，`tauri dev` 场景见下）。
- Node ≥ 18，Rust ≥ 1.75。

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

## 打包（UOS20 / Debian10 容器内执行）

**完整离线打包指引见 [`packaging/README.md`](packaging/README.md)**，配套资产：

- `packaging/Dockerfile` — Debian10 构建镜像（webkit2gtk-4.0 + Rust 1.79 + Node18）
- `packaging/build-deb.sh` — 容器内一键构建脚本（全部 `--offline`）

`tauri.conf.json` 已按 UOS20 预配置（deb depends 含 `libwebkit2gtk-4.0-37`）。

## 数据落盘位置

- 巡检台账：`~/.local/share/ir-inspection-tool/ledger.db`（Windows：`%APPDATA%`）
- 阈值配置：应用配置目录 `thresholds.json`

## 目录结构

```
├── src/                    # Vue3 前端
│   ├── api/tauri.js        # IPC 封装
│   ├── stores/             # pinia：image/roi/batch/ledger/settings
│   ├── utils/              # 调色板 LUT、缺陷等级、格式化
│   └── components/         # CanvasView/ColorScale/RoiPanel/MetaPanel/
│                           # BatchPanel/LedgerPanel/ReportDialog/SettingsDialog
├── src-tauri/              # Rust 后端
│   └── src/
│       ├── main.rs         # 命令注册 + 配置初始化
│       ├── thermal.rs      # R-JPEG 解析 + EXIF（thermal crate + 大疆适配）
│       ├── temperature.rs  # 普朗克换算（ε/距离 serde 往返修正）
│       ├── roi.rs          # ROI 掩码统计（矩形/多边形/线段）
│       ├── diagnosis.rs    # 四级判定 + 三相 ΔT（F11/F12）
│       ├── export.rs       # CSV / 标注 PNG（F8）
│       ├── db.rs           # SQLite 台账（F15）
│       ├── report.rs       # docx 缺陷报告（F16）
│       ├── batch.rs        # 文件夹批量解析（F14）
│       └── settings.rs     # 阈值持久化（F12）
├── scripts/                # e2e-cdp.mjs（mock 验收）、e2e-shot.ps1（真机截图）
└── e2e/sample.json         # dump_temps 导出的真实样本温度矩阵
```

## 测试与验收

一条命令跑通全部自动化验收（零新增依赖，不改动离线打包配置）：

```bash
pnpm test                 # 全部通道
pnpm test:rust            # Rust 单测 + 真实样本全链路
pnpm test:frontend        # 前端纯逻辑
pnpm test:sample          # 样本基线
pnpm test:e2e             # E2E 现状核查
```

| 通道 | 实际命令 | 覆盖 |
| --- | --- | --- |
| Rust | `cargo test --offline`（`src-tauri/backend-tests`） | 32 项：R-JPEG 解析 / 测温 / ROI / 定级 / 台账 / 导出 / 批量 / 阈值，含两张真实辐射原图 |
| 前端 | `node --test tests/*.test.js` | 20 项：调色板 LUT、缺陷等级、格式化、19 个 IPC 命令契约、5 个 pinia store |
| 样本基线 | `node scripts/verify-sample.mjs` | `e2e/sample.json` 的 15 字段、640×512 矩阵、ε / 距离 / 温度量程 |
| E2E | `node scripts/check-e2e.mjs` | 静态核对 19 个注册命令 / 19 个前端封装 / 9 个 mock 分支 |

### 为什么后端测试放在 `src-tauri/backend-tests/`

主 crate 的任意 cargo target 都会链接 tauri → `webkit2gtk-4.0` / `gtk-3` / `glib-2.0` 开发库；精简开发机与内网 CI 通常只有运行时库（例如本机仅有 4.1），`gdk-sys` / `gio-sys` 的 build script 会直接失败。独立 crate 沿用 `bin/dump_temps.rs` 的 `#[path]` 做法，复用**同一份**源码模块与内置 `#[cfg(test)]` 用例，不链接 tauri，因此完全离线、无 GTK 依赖。开发环境若已装齐 GTK 4.0 开发库，仍可额外执行 `cd src-tauri && cargo test --lib` 验证完整 lib 目标。

### 工具链与样本

- 入口脚本自动把 `~/.cargo/bin` 加入 PATH；Node < 18 时自动从 NVM 目录（`NVM_DIR`，默认 `~/.config/nvm`）挑选最高版本自举。
- 真实样本经 `IR_SAMPLE_JPG` / `IR_SAMPLE_DJI` 注入（也写在 `backend-tests/.cargo/config.toml`）；样本缺失时测试**直接失败**，不静默跳过。
- 测试临时目录均带进程号与时间戳，支持 `cargo test` 并行与重复执行。

### 验收记录（2026-11-04）

- Rust 32/32、前端 20/20、样本基线、E2E 静态核查全部通过。
- 真实样本 `DJI_20260804234634_0002_T.JPG`：640×512、ε=0.95、距离 5 m、28.96~32.70 ℃；CSV / 标注 PNG / Word 报告产物均可生成并解码。
- `scripts/e2e-cdp.mjs` 因本机无 Chrome/Chromium 且默认路径为 Windows，CDP 图形验收记为**环境受限项**；在有浏览器的机器设置 `CHROME_PATH` 并先启动 `pnpm dev` 后可复现。

## 已知事项

- Windows-GNU 下 `cargo test` 的测试二进制存在 WebView2Loader 符号解析问题
  （应用本体与 Linux 目标不受影响）；单元测试在 Linux/CI 上运行，现统一走无 GTK 依赖的 `backend-tests`。
- headless 验收脚本中 Vue 异步 flush 存在延迟，脚本已用强制刷新兜底。
- 批量解析为顺序执行（100 张内无感），后续可换 rayon 并行。
