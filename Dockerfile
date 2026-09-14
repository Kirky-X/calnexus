# CalNexus HTTP/MCP server 多阶段构建
# 构建：docker build -t calnexus:latest .
# 运行：docker run -p 3000:3000 calnexus:latest --serve-http --bind 0.0.0.0:3000

# ---- 构建阶段 ----
FROM rust:1.85-slim AS builder
RUN apt-get update && apt-get install -y --no-install-recommends protoc && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY locales ./locales
# 依赖缓存层（无 lock 变更时复用）
RUN mkdir -p src/bin && echo "fn main() {}" > src/bin/dummy.rs || true
RUN cargo build --release --features server --bin calnexus || cargo build --release --features server

# ---- 运行阶段 ----
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /build/target/release/calnexus /usr/local/bin/calnexus
# 非 root 用户（distroless cc 自带 nonroot 用户 uid:65532）
USER nonroot:nonroot
ENV CALNEXUS_BIND_ADDR=0.0.0.0:3000
EXPOSE 3000
# graceful shutdown：SIGTERM 直达进程（无 shell 包装）
ENTRYPOINT ["/usr/local/bin/calnexus"]
CMD ["--serve-http"]
