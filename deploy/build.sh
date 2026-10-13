#!/usr/bin/env bash
set -euo pipefail

# rss-manager 一键构建脚本（交付物 D-05）
# 用法：./deploy/build.sh [binary|docker]
#   binary  → rspack 构建前端 → cargo build --release → 产出 release 二进制（前端 rust-embed 内嵌）
#   docker  → docker build 产出单容器镜像

MODE="${1:-binary}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

case "$MODE" in
  binary)
    echo "==> 构建前端 (rspack)"
    (cd frontend && npm ci && npm run build)
    echo "==> 构建后端 (cargo build --release)"
    cargo build --manifest-path backend/Cargo.toml --locked --release
    echo "==> 产物：backend/target/release/rss-manager"
    ;;
  docker)
    echo "==> 构建镜像 (docker build)"
    docker build -f deploy/Dockerfile -t rss-manager:latest .
    echo "==> 产物：rss-manager:latest"
    ;;
  *)
    echo "用法：$0 [binary|docker]" >&2
    exit 1
    ;;
esac
