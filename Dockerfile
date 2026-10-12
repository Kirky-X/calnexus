# CalNexus HTTP/MCP server 多阶段构建
# 构建：docker build -t calnexus:latest .
# 运行：docker run -p 3000:3000 calnexus:latest --serve-http --bind 0.0.0.0:3000

# ---- 构建阶段 ----
# 基础镜像 = crate MSRV（Cargo.toml rust-version = 1.97.1），标签与运行阶段同为 bookworm
# 以保持 glibc 版本一致（distroless/cc-debian12 = bookworm / glibc 2.36）
FROM rust:1.97-slim-bookworm AS builder
# 包名是 protobuf-compiler（提供 /usr/bin/protoc 供 prost/tonic 代码生成）
RUN apt-get update && apt-get install -y --no-install-recommends protobuf-compiler && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY locales ./locales
# tests/ 与 benches/ 是 Cargo.toml 显式声明的 target 路径，cargo 解析 manifest 时要求文件存在
COPY tests ./tests
COPY benches ./benches
# 服务入口 --serve-http 由 cli feature 提供（cli 未启用时二进制仅报 feature 缺失并退出 2）
RUN cargo build --release --features cli,server --bin calnexus

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
