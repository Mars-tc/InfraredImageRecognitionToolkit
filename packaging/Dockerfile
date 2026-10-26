# Debian10 (buster) 构建镜像 —— UOS20 同源，避免高版本 glibc 污染产物
# 用法（在能联网的 Linux 机器上）:
#   docker build -t ir-build -f packaging/Dockerfile .
#   docker run --rm -v $(pwd):/app ir-build /app/packaging/build-deb.sh
FROM debian:buster

ENV DEBIAN_FRONTEND=noninteractive

RUN sed -i 's|deb.debian.org|mirrors.aliyun.com|g; s|security.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list && \
    apt-get update && apt-get install -y --no-install-recommends \
      build-essential curl ca-certificates file libssl-dev \
      libwebkit2gtk-4.0-dev librsvg2-dev libgtk-3-dev libayatana-appindicator3-dev \
      python3-minimal xz-utils && \
    rm -rf /var/lib/apt/lists/*

# Rust 工具链（固定版本，保证产物可复现；可按需调整）
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH=/usr/local/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.79.0 --profile minimal

# Node 18 二进制包
RUN curl -fsSL https://nodejs.org/dist/v18.20.4/node-v18.20.4-linux-x64.tar.xz \
      -o /tmp/node.tar.xz && tar -xJf /tmp/node.tar.xz -C /usr/local --strip-components=1 && rm /tmp/node.tar.xz && \
    npm i -g pnpm@10
