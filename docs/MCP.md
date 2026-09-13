# CalNexus MCP 集成指南（v015 新增）

CalNexus 通过 [Model Context Protocol](https://modelcontextprotocol.io) 将数学求值能力暴露给 AI 客户端（Claude Desktop、Cursor、任意 MCP 客户端）。传输为 stdio，无会话状态（stateless）。

## 客户端配置

### Claude Desktop

`claude_desktop_config.json`：

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

### 通用 stdio 客户端

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

## 工具

| 工具 | 参数（req 包装） | 返回 |
| --- | --- | --- |
| `evaluate` | `expr`（必填）、`vars`（可选）、`precision`（可选） | `{result, domain, cache}` |
| `list_functions` | 无（`{"req": {}}`） | 按域分组的函数目录（含 feature 门控域可见性） |
| `fx_budget` * | `tuition`、`tuition_currency`、`home_currency`、`living_cost_monthly`?、`duration_years` | 本币费用 + ±3% 风险区间 + `rate_date` |
| `fx_pricing` * | `cost_cny`、`target_profit_rate`、`currencies`、`platform_rate`?、`safety_buffer`? | 各币种建议售价 + `rate_date` |

\* 需 `fx` feature 构建。

## 参数包装约定（重要）

所有参数必须包在 `req` 对象内：

```json
{"req": {"expr": "diff(x^2, x)"}}
{"req": {"expr": "x+1", "vars": {"x": 10}}}
```

未包装的 `{"expr": "..."}` 会被 schema 校验拒绝。

## 错误语义

| HTTP | MCP | 含义 |
| --- | --- | --- |
| 400 | InvalidInput | 语法错误、除零、定义域等求值失败 |
| 422 | ValidationError | 参数校验失败（expr ≤ 4096 字符、vars ≤ 1024 键、precision ≤ 10000） |
| 503 | ServiceUnavailable | 计算超时或上游（fx 汇率源）不可达，带 `Retry-After` |

## 发现可用函数

调用 `list_functions` 工具（或 CLI `calnexus --list-functions`）获取当前构建的完整函数目录；feature 门控域（time/unit/fx）仅在启用时出现。
