# CalNexus 部署指南（v015 新增）

## 本机运行

```bash
cargo install calnexus --features cli
calnexus --serve-http                # 默认 127.0.0.1:3000
calnexus --serve-http --bind 0.0.0.0:8080
```

## Docker

```bash
docker build -t calnexus:latest .
docker run -p 3000:3000 calnexus:latest
```

容器内默认 `CALNEXUS_BIND_ADDR=0.0.0.0:3000`（对外暴露请置于反代/Ingress 之后）。

## Kubernetes 要点

- **readinessProbe**: `GET /ready`（依赖就绪语义：进程内缓存可读）
- **livenessProbe**: `GET /live`（纯进程存活语义）
- **terminationGracePeriodSeconds**: ≥ 45（请求级超时 30s + 排空余量 15s；
  服务端 drain 超时固定 30s，超时后进程强退）
- **资源**: 求值为 CPU 密集型；缓存字节预算经 `CALNEXUS_CACHE_SIZE`（条目 × 4KB 近似）控制

## 可观测性

- `GET /metrics`：Prometheus 文本（`calnexus_cache_*` + `calnexus_http_requests_total`）
- `X-Request-ID`：入站透传或自动生成（`req-*`），响应回写；`traceparent` 同样透传
- `observability` feature 构建时，`RUST_LOG` 控制日志级别（默认 warn）

## 错误语义（SLO 友好）

| 状态码 | 语义 | 触发 |
| --- | --- | --- |
| 400 | InvalidInput（客户端输入/求值错误） | 语法错误、除零、定义域等 |
| 422 | ValidationError（参数校验失败） | expr/vars/precision 超限 |
| 503 | ServiceUnavailable（服务端/上游故障） | 计算超时、fx 上游不可达（`ErrorKind::DependencyUnavailable`），带 `Retry-After` |
