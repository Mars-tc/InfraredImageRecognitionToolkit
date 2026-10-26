# UOS20 deb 打包指引（内网离线）

## 原理

- 目标机 UOS20 = Debian10 底层，**必须在 Debian10 环境编译**，否则高版本 glibc 产物无法安装运行。
- 内网无网络：Rust/npm 依赖需提前在联网侧「打包携带」，构建时全部 `--offline`。

## 步骤一：联网侧准备离线依赖（本机或任意联网 Linux）

```bash
# 1. Rust vendor（把全部 crate 源码复制到 src-tauri/vendor/）
cd src-tauri
cargo vendor vendor          # 结束后会打印 [source] 配置片段，原样保存
# 按 cargo vendor 输出创建 src-tauri/.cargo/config.toml：
#   [source.crates-io]
#   replace-with = "vendored-sources"
#   [source.vendored-sources]
#   directory = "vendor"

# 2. pnpm 离线 store（在项目根）
pnpm install --lockfile-only
pnpm fetch                    # 拉取全部依赖到本地 store
pnpm store path               # 记住 store 路径，下一步打包携带
```

## 步骤二：构建 deb（Debian10 容器）

```bash
# 在能跑 Docker 的机器上：
docker build -t ir-build -f packaging/Dockerfile .
docker run --rm -v $(pwd):/app ir-build /app/packaging/build-deb.sh
# 产物：src-tauri/target/release/bundle/deb/ir-inspection-tool_0.1.0_amd64.deb
```

无 Docker 的替代：在 UOS20/Debian10 机器上直接装好
`libwebkit2gtk-4.0-dev build-essential libssl-dev librsvg2-dev` 与 Node18+pnpm、
Rust（rustup 离线安装包），然后按 build-deb.sh 相同步骤执行。

## 步骤三：内网分发

```bash
sudo dpkg -i ir-inspection-tool_0.1.0_amd64.deb
# 缺 libwebkit2gtk-4.0-37 时，先从 UOS20 官方源离线下载该包及依赖再安装
```

## 常见问题

| 现象 | 处理 |
| --- | --- |
| `GLIBC_2.XX not found` | 编译环境不是 Debian10，换容器重编 |
| `webkit2gtk-4.0` 找不到 | 确认装了 `libwebkit2gtk-4.0-dev`（4.0 不是 4.1） |
| cargo 拉取 crates.io | `.cargo/config.toml` 未生效，确认在 `src-tauri/` 下且 directory 路径正确 |
| pnpm install 联网 | 少了 `--offline` 或 store 未就位 |
