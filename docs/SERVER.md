# 🖥️ CalNexus 服务模式指南

> 本文档覆盖服务模式的运行、部署、可观测性与 MCP 接入。
> 服务模式需以 `--features server` 启用（`server = http + mcp`）。

## 📋 目录

- [🚀 快速启动](#-快速启动)
- [🌐 HTTP API 与运维端点](#-http-api-与运维端点)
- [🚢 部署](#-部署)
  - [本机运行](#本机运行)
  - [Docker](#docker)
  - [Kubernetes 要点](#kubernetes-要点)
- [📈 可观测性](#-可观测性)
- [🔌 MCP 集成](#-mcp-集成)
  - [客户端配置](#客户端配置)
  - [工具](#工具)
  - [参数包装约定（重要）](#参数包装约定重要)
  - [发现可用函数](#发现可用函数)
- [🚨 错误语义（SLO 友好）](#-错误语义slo-友好)
- [📚 相关文档](#-相关文档)

---

## 🚀 快速启动

```bash
cargo install calnexus --features server
calnexus --serve-http                    # 默认 127.0.0.1:3000
calnexus --serve-http --bind 0.0.0.0:8080
```

---

## 🌐 HTTP API 与运维端点

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/v1/evaluate` | POST | 表达式求值（JSON 请求/响应） |
| `/api/v1/list_functions` | POST | 运行时函数目录（含 feature 门控域可见性） |
| `/health` | GET | 综合健康检查（含 L1 缓存状态） |
| `/live` | GET | 存活探针（liveness，纯进程存活语义） |
| `/ready` | GET | 就绪探针（readiness，依赖就绪语义） |
| `/metrics` | GET | 指标导出（Prometheus 文本，`?format=json` 返回 JSON） |

**语言协商**：`evaluate`、`fx_budget`、`fx_pricing` 请求体均支持可选 `lang` 字段（BCP-47 标签，如 `"en"`/`"zh-CN"`）。缺省或未知值回退英文（协议默认）；`"zh"` 时错误消息与 fx 风险提示等人类可读文案切换中文，机器可读字段（`type` 协议名等）保持英文契约。MCP tool args 同构支持 `lang`：

```bash
curl -X POST localhost:3000/api/v1/evaluate -H 'content-type: application/json' \
  -d '{"expr": "foo + 1", "lang": "zh"}'
# → {"type":"InvalidInput","message":"求值错误: 未绑定变量: foo",...}
```

`docs` feature 提供 Swagger UI（`/swagger-ui`，OpenAPI 文档）。

---

## 🚢 部署

### 本机运行

```bash
calnexus --serve-http                # 默认 127.0.0.1:3000
calnexus --serve-http --bind 0.0.0.0:8080
```

### Docker

```bash
docker build -t calnexus:latest .
docker run -p 3000:3000 calnexus:latest
```

容器为多阶段 distroless 非 root 镜像，默认 `CALNEXUS_BIND_ADDR=0.0.0.0:3000`（对外暴露请置于反代/Ingress 之后）。

### Kubernetes 要点

- **readinessProbe**: `GET /ready`（依赖就绪语义：进程内缓存可读）
- **livenessProbe**: `GET /live`（纯进程存活语义）
- **terminationGracePeriodSeconds**: ≥ 45（请求级超时 30s + 排空余量 15s；服务端 drain 超时固定 30s，超时后进程强退）
- **资源**: 求值为 CPU 密集型，副本数按核数扩展优于调大缓存；缓存字节预算经 `CALNEXUS_CACHE_SIZE`（条目 × 4KB 近似）控制

优雅关闭随 `http` feature 内建：SIGTERM/Ctrl+C → drain 最长 30s → 强退。

---

## 📈 可观测性

- `GET /metrics`：Prometheus 文本（`calnexus_cache_*` + `calnexus_http_requests_total`）
- `X-Request-ID`：入站透传或自动生成（`req-*`），响应回写；`traceparent` 同样透传（提取 trace_id 回写 `X-Trace-ID`）
- `observability` feature 构建时，`RUST_LOG` 控制日志级别（默认 warn）
- `ratelimit` feature 提供固定窗口 per-IP 限流：`CALNEXUS_RATELIMIT_LIMIT`（默认 120）、`CALNEXUS_RATELIMIT_WINDOW_SECS`（默认 60），探针与 metrics 豁免

---

## 🔌 MCP 集成

`calnexus --serve-mcp` 以 stdio 传输暴露 MCP 工具，无会话状态（stateless），可接入 Claude Desktop、Cursor 等任意 MCP 客户端。

### 客户端配置

Claude Desktop 的 `claude_desktop_config.json`：

```json
{
  "mcpServers": {
    "calnexus": {
      "command": "calnexus",
      "args": ["--serve-mcp"]
    }
  }
}
```

> `calnexus` 需在 PATH 中（`cargo install calnexus --features cli`），或替换为绝对路径。
> `fx` feature 构建会额外注册 `fx_budget`/`fx_pricing` 工具。

通用 stdio 客户端：

```json
{
  "mcpServers": {
    "calnexus": {
      "command": "/usr/local/bin/calnexus",
      "args": ["--serve-mcp"],
      "env": {}
    }
  }
}
```

### 工具

| 工具 | 参数（req 包装） | 返回 |
|------|------------------|------|
| `evaluate` | `expr`（必填）、`vars`（可选）、`precision`（可选）、`lang`（可选） | `{result, domain, cache}` |
| `list_functions` | 无（`{"req": {}}`） | 按域分组的函数目录（含 feature 门控域可见性） |
| `fx_budget` * | `tuition`、`tuition_currency`、`home_currency`、`living_cost_monthly`?、`duration_years` | 本币费用 + ±3% 风险区间 + `rate_date` |
| `fx_pricing` * | `cost_cny`、`target_profit_rate`、`currencies`、`platform_rate`?、`safety_buffer`? | 各币种建议售价 + `rate_date` |

\* 需 `fx` feature 构建。

### 参数包装约定（重要）

所有参数必须包在 `req` 对象内：

```json
{"req": {"expr": "diff(x^2, x)"}}
{"req": {"expr": "x+1", "vars": {"x": 10}}}
```

未包装的 `{"expr": "..."}` 会被 schema 校验拒绝。

### 发现可用函数

调用 `list_functions` 工具（或 CLI `calnexus --list-functions`、HTTP `POST /api/v1/list_functions`）获取当前构建的完整函数目录；feature 门控域（time/unit/fx）仅在启用时出现。

---

## 🚨 错误语义（SLO 友好）

服务端错误三分类与 CLI 退出码出自同一 `ErrorKind` 契约；注意 422 来自独立的请求校验层，而非求值错误：

| HTTP | MCP | 语义 | 触发来源 |
|------|-----|------|----------|
| 400 `InvalidInput` | INVALID_INPUT | 求值/解析失败 | 9 种求值类 `ErrorKind`（Parse / Eval / Overflow / DivisionByZero / Domain / Depth / NaNOrInf / UndefinedSymbol / Usage），message 以 kind 名为前缀 |
| 422 `ValidationError` | VALIDATION_ERROR | 请求体参数校验失败 | DTO 校验层（`server/types.rs::validate()`）：expr ≤ 4096 字符、vars ≤ 1024 键、precision ≤ 10000；在求值之前拦截 |
| 503 `ServiceUnavailable` | SERVICE_UNAVAILABLE | 服务端/上游故障 | `Timeout` / `DependencyUnavailable`（fx 上游不可达），带 `Retry-After` |

CLI 侧退出码（`ErrorKind::exit_code()` 契约）：**1** = 计算错误、**2** = 用法错误（`Usage`）、**3** = 超时/上游不可用。422 校验层为 HTTP/MCP 形态独有，CLI 形态的对应约束由 clap / `--precision` 校验承担。

---

## 📚 相关文档

- [📖 用户指南](USER_GUIDE.md) — 环境变量与 CLI 旗标
- [📈 性能指南](PERFORMANCE.md) — 缓存预算与容量规划
- [🔒 安全文档](SECURITY.md) — 服务化安全最佳实践
- [📘 API 参考](API_REFERENCE.md) — 错误契约与 `ErrorKind` 全表
- [🏗️ 架构文档](ARCHITECTURE.md) — server 模块与数据流
