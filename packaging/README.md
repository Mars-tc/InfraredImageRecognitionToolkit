# 交付构建与内网安装（GitHub Actions）

交付给统信 UOS20（Debian 10 底层）的安装包由 **GitHub Actions** 统一产出，本机不再维护 Docker 构建链。

## 为什么要云端构建

| 构建环境 | glibc | 能否安装到 UOS20 |
| --- | --- | --- |
| 交付构建（Debian 10 / buster 容器） | 2.28 | 可以 |
| 开发机直接构建（当前开发机为 Deepin 25） | 2.38 | 不可以，会报 `GLIBC_2.XX not found` |

AppImage 同样依赖宿主的 glibc（不自带 glibc），因此也必须在 Debian 10 环境构建。

## 一、触发构建

- **手动**：仓库 Actions 页 → `build-uos20` → `Run workflow`。
- **自动**：push 到 `main` 或提 PR 做构建校验；推送 `v*` 标签（如 `v0.1.0`）会额外发布 GitHub Release。
- 两种方式的产物都会作为 artifact（`uos20-installers`）保留 30 天。

## 二、流水线做了什么（`.github/workflows/build-uos20.yml`）

1. 在 `debian:buster` 容器内构建（buster 已 EOL，脚本自动把 apt 源切到 `archive.debian.org`）；
2. 安装 Node 20.19+（Vite 8 要求 `^20.19 || >=22.12`）、`pnpm@10.24.0`、Rust 1.90（`image 0.25` 要求 ≥ 1.88）；
3. `pnpm install --frozen-lockfile` 后用 cargo 版 tauri CLI 执行 `cargo tauri build --bundles deb`（`cargo install tauri-cli --version 1.8.3 --locked`，与 `Cargo.lock` 中的 tauri crate 同版本；npm 版 CLI 1.x 止于 1.6.3，会被版本校验拒绝）；
4. 校验产物依赖的最高 GLIBC 符号版本不得超过 2.28，超出即判定构建失败；
5. 额外尝试打包 AppImage（可选产物，失败不阻塞 deb）；
6. 产物重命名为 ASCII 文件名并生成 `SHA256SUMS`。

## 三、产物

| 文件 | 说明 |
| --- | --- |
| `ir-inspection-tool_0.1.0_amd64.deb` | UOS20 离线安装包（必需产物） |
| `ir-inspection-tool_0.1.0_amd64.AppImage` | 免安装可执行包（可选产物，视 linuxdeploy 在容器内的可用性） |
| `SHA256SUMS` | 校验和 |

## 四、内网分发与安装

```bash
# 1. 在联网侧从 GitHub 下载产物，拷贝进内网
sha256sum -c SHA256SUMS

# 2. UOS20 内网机离线安装
sudo dpkg -i ir-inspection-tool_0.1.0_amd64.deb
# 若提示缺 libwebkit2gtk-4.0-37，先从 UOS20 官方源离线拉取该包及依赖再安装

# 3. AppImage 方式（免安装）
chmod +x ir-inspection-tool_0.1.0_amd64.AppImage
./ir-inspection-tool_0.1.0_amd64.AppImage
```

安装后的应用仍然**纯离线运行**：不访问任何外部服务，离线点位地图为纯 SVG 绘制、无底图瓦片。构建过程本身需要联网拉取依赖，但产物不携带任何运行时外网依赖。

## 五、本机开发构建

交付包不在本机构建，本机只做开发与调试：

> 首次使用需安装与 `Cargo.lock` 中 tauri crate（1.8.3）同版本的 tauri CLI：
> `cargo install tauri-cli --version 1.8.3 --locked`

```bash
node scripts/build-local.mjs env      # 环境自检
node scripts/build-local.mjs install  # 安装前端依赖
node scripts/build-local.mjs dev      # 启动 Tauri 开发模式
node scripts/build-local.mjs build    # 本机调试包（不可交付 UOS20）
node scripts/build-local.mjs test     # 跑全部自动化验收
```

脚本会自动把 `~/.cargo/bin` 加入 PATH，并在 pnpm 缺失时回退到 corepack。详见仓库 README「开发环境运行」章节。

## 常见问题

| 现象 | 处理 |
| --- | --- |
| 构建在 GLIBC 断言步骤失败 | 构建环境被改动，产物不再兼容 UOS20；确认流水线仍使用 `debian:buster` |
| deb 安装提示缺 `libwebkit2gtk-4.0-37` | 从 UOS20 官方源离线拉取该包及依赖后重装 |
| 没有 AppImage 产物 | linuxdeploy 在容器内不可用时属预期，deb 为必需产物 |
| 本机 `pnpm` 找不到 | 执行 `corepack enable pnpm`（按 package.json 的 packageManager 自动准备 pnpm@10.24.0） |
| 提示 tauri CLI 与 crate 版本不兼容 | npm 的 `@tauri-apps/cli` 1.x 止于 1.6.3，改用 cargo 版 `cargo install tauri-cli --version 1.8.3 --locked` |
| 本机 `cargo` 找不到 | `build-local.mjs` 已自动注入 `~/.cargo/bin`，或手动 `export PATH="$HOME/.cargo/bin:$PATH"` |
