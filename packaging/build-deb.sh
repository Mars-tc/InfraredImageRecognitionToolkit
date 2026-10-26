#!/usr/bin/env bash
# 在 Debian10 环境（容器或 UOS20 机器）内构建 deb 安装包
# 前提：/app 为项目根（已挂载），依赖已离线准备好（见 packaging/README.md）
set -euo pipefail
cd /app

echo "==> [1/3] 安装前端依赖（离线）"
pnpm install --offline --frozen-lockfile

echo "==> [2/3] 构建前端产物"
pnpm build

echo "==> [3/3] Rust 编译 + deb 打包"
cd src-tauri
cargo build --release --offline
cargo tauri build --bundles deb

echo "==> 完成，产物："
ls -lh target/release/bundle/deb/*.deb
