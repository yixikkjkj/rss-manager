# Build from the project root: docker build -f deploy/Dockerfile .
# Frontend and backend are independent source directories; the release binary embeds the UI.
FROM node:24-alpine AS frontend
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/rspack.config.js frontend/tsconfig.json frontend/index.html ./
COPY frontend/src ./src
RUN npm run build

FROM rust:1.95-slim-bookworm AS builder
WORKDIR /app/backend
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/migrations ./migrations
COPY backend/src ./src
COPY --from=frontend /app/frontend/dist /app/frontend/dist
RUN cargo build --locked --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 1000 rssmanager
WORKDIR /app
COPY --from=builder /app/backend/target/release/rss-manager /app/rss-manager
RUN mkdir -p /data/backups && chown -R rssmanager:rssmanager /data
USER rssmanager
ENV RSS_MANAGER_DB_PATH=/data/rss_manager.db \
    RSS_MANAGER_BACKUP_DIR=/data/backups \
    RSS_MANAGER_LISTEN_ADDR=0.0.0.0:8080
EXPOSE 8080
VOLUME ["/data"]
CMD ["/app/rss-manager"]
