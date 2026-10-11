// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Server 接口层 DTO：请求/响应类型 + EvalResult→JSON 转换。
//!
//! 错误契约：`ApiError`（InvalidInput→400 / ValidationError→422）。

use crate::core::{EvalContext, EvalResult, MAX_PRECISION};
use crate::domains::format_bigrational;
use sdforge::error::ApiError;
use std::collections::HashMap;

/// HTTP/MCP 求值请求（反序列化 `{"expr","vars","precision","lang"}`）。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct EvaluateRequest {
    /// 表达式字符串（必填）。
    pub expr: String,
    /// 变量绑定（可选，默认空）。
    #[serde(default)]
    pub vars: HashMap<String, f64>,
    /// 任意精度位数（可选，None=常规模式）。
    #[serde(default)]
    pub precision: Option<usize>,
    /// 响应语言（BCP-47 标签，缺省/未知回退英文）；仅影响人可读文案，协议字段保持英文。
    #[serde(default)]
    pub lang: Option<String>,
}

/// HTTP/MCP 求值响应（序列化 `{"result","domain","cache"}`）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct EvaluateResponse {
    /// 求值结果（JSON 值，类型取决于 EvalResult 变体）。
    pub result: serde_json::Value,
    /// 计算域名（如 "arithmetic"、"number_theory"）。
    pub domain: String,
    /// 缓存状态："hit" 或 "miss"。
    pub cache: String,
}

/// Server 启动错误。
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// HTTP server 启动/运行错误。
    #[error("HTTP server error: {0}")]
    Http(String),
    /// MCP server 启动/运行错误。
    #[error("MCP server error: {0}")]
    Mcp(String),
}

impl EvaluateRequest {
    /// 将请求转换为 EvalContext（供 evaluate 函数使用）。
    pub fn to_eval_context(&self) -> EvalContext {
        EvalContext {
            vars: self.vars.clone(),
            precision: self.precision,
            ..Default::default()
        }
    }

    /// 校验请求安全约束（expr≤4096 / vars≤1024 / 键名≤64 / 非有限值拒绝 / precision≤10000）；违规返回 HTTP 422 / MCP VALIDATION_ERROR。
    // ApiError 体积为框架设计；校验错误路径罕见，Box 化会迫使所有 `validate()?` 调用点解包
    #[allow(clippy::result_large_err)]
    pub fn validate(&self) -> Result<(), ApiError> {
        // expr 字段校验（空串 / 超长 / null 字节均为 DoS / 注入向量）
        if self.expr.is_empty() {
            return Err(ApiError::validation("expr", "expression must not be empty"));
        }
        if self.expr.len() > MAX_EXPR_LEN {
            return Err(ApiError::validation(
                "expr",
                format!("length {} exceeds limit {}", self.expr.len(), MAX_EXPR_LEN),
            ));
        }
        if self.expr.bytes().any(|b| b == 0) {
            return Err(ApiError::validation("expr", "null bytes not allowed"));
        }
        if self.vars.len() > MAX_VARS {
            return Err(ApiError::validation(
                "vars",
                format!("size {} exceeds limit {}", self.vars.len(), MAX_VARS),
            ));
        }
        // vars 键名校验：超长/含 null 字节为内存攻击 / 注入向量（与 expr 校验对齐）
        for name in self.vars.keys() {
            if name.len() > MAX_VAR_NAME_LEN {
                return Err(ApiError::validation(
                    "vars",
                    format!(
                        "var name length {} exceeds limit {}",
                        name.len(),
                        MAX_VAR_NAME_LEN
                    ),
                ));
            }
            if name.bytes().any(|b| b == 0) {
                return Err(ApiError::validation(
                    "vars",
                    "var name null bytes not allowed",
                ));
            }
        }
        // NaN/Infinity 显式校验：确保 validate() 自包含，不依赖反序列化层拦截
        if self.vars.values().any(|v| !v.is_finite()) {
            return Err(ApiError::validation(
                "vars",
                "values must be finite (NaN/Infinity not allowed)",
            ));
        }
        if let Some(p) = self.precision
            && p > MAX_PRECISION
        {
            return Err(ApiError::validation(
                "precision",
                format!("{} exceeds limit {}", p, MAX_PRECISION),
            ));
        }
        Ok(())
    }
}

/// `vars` 最大键数（安全前置任务：防止内存耗尽攻击）。
const MAX_VARS: usize = 1024;

/// `vars` 单个键名最大长度（防止键名注入 / 内存攻击）。
const MAX_VAR_NAME_LEN: usize = 64;

/// `expr` 最大长度（防止超长表达式导致 DoS）。
const MAX_EXPR_LEN: usize = 4096;

impl EvaluateResponse {
    /// 从 evaluate 返回值构造响应；BigRational + `fmt_prec` 时格式化为十进制字符串。
    pub fn from_eval(
        result: EvalResult,
        domain: String,
        cache_hit: bool,
        fmt_prec: Option<usize>,
    ) -> Self {
        let result_json = match (&result, fmt_prec) {
            (EvalResult::BigRational(r), Some(p)) => {
                serde_json::Value::from(format_bigrational(r, Some(p)))
            }
            _ => eval_result_to_json(&result),
        };
        Self {
            result: result_json,
            domain,
            cache: if cache_hit {
                "hit".to_string()
            } else {
                "miss".to_string()
            },
        }
    }
}

/// 将 EvalResult 转换为 JSON（NaN/Infinity 与 BigInt 用 String 表示）。
fn eval_result_to_json(result: &EvalResult) -> serde_json::Value {
    use serde_json::{Value, json};
    match result {
        EvalResult::Scalar(v) => {
            if v.is_finite() {
                // 整数值 f64 转 i64，使 `5.0` 序列化为 `5` 而非 `5.0`
                if v.fract() == 0.0 && v.abs() < i64::MAX as f64 {
                    Value::from(*v as i64)
                } else {
                    Value::from(serde_json::Number::from_f64(*v).unwrap_or_else(|| {
                        // from_f64 返回 None 仅当 NaN/Infinity，但前面已过滤 is_finite
                        serde_json::Number::from(0)
                    }))
                }
            } else {
                // NaN/Infinity：JSON 无对应类型，用 String 表示
                Value::from(v.to_string())
            }
        }
        EvalResult::Complex(re, im) => {
            json!({"re": re, "im": im})
        }
        EvalResult::Matrix(m) => Value::Array(
            m.iter()
                .map(|row| Value::Array(row.iter().map(|v| Value::from(*v)).collect()))
                .collect(),
        ),
        EvalResult::BigInt(b) => Value::from(b.to_string()),
        EvalResult::BigRational(r) => {
            json!({
                "num": r.numer().to_string(),
                "den": r.denom().to_string(),
            })
        }
        EvalResult::Vector(v) => Value::Array(v.iter().map(|x| Value::from(*x)).collect()),
        EvalResult::Polynomial(p) => Value::Array(p.iter().map(|x| Value::from(*x)).collect()),
        EvalResult::ComplexList(l) => Value::Array(
            l.iter()
                .map(|(re, im)| json!({"re": re, "im": im}))
                .collect(),
        ),
        EvalResult::Symbolic(s) => Value::from(s.as_str()),
        EvalResult::LaTeX(s) => Value::from(s.as_str()),
        EvalResult::Steps(v) => Value::Array(v.iter().map(|s| Value::from(s.as_str())).collect()),
        EvalResult::Json(v) => v.clone(),
        // DateTime 带 "type" 标签，便于客户端与 Symbolic/LaTeX 纯字符串结果区分
        EvalResult::DateTime(s) => json!({"type": "datetime", "value": s}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;
    use num_rational::BigRational;

    // === EvaluateRequest 反序列化 ===

    #[test]
    fn test_evaluate_request_minimal() {
        let json = r#"{"expr":"2+3"}"#;
        let req: EvaluateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.expr, "2+3");
        assert!(req.vars.is_empty());
        assert_eq!(req.precision, None);
    }

    #[test]
    fn test_evaluate_request_with_vars() {
        let json = r#"{"expr":"x+1","vars":{"x":10.0}}"#;
        let req: EvaluateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.expr, "x+1");
        assert_eq!(req.vars.get("x"), Some(&10.0));
        assert_eq!(req.precision, None);
    }

    #[test]
    fn test_evaluate_request_with_precision() {
        let json = r#"{"expr":"1/3","precision":2}"#;
        let req: EvaluateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.expr, "1/3");
        assert_eq!(req.precision, Some(2));
    }

    #[test]
    fn test_evaluate_request_precision_null() {
        let json = r#"{"expr":"2+3","precision":null}"#;
        let req: EvaluateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.precision, None);
    }

    #[test]
    fn test_evaluate_request_to_eval_context() {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 5.0);
        let req = EvaluateRequest {
            expr: "x+1".to_string(),
            vars,
            precision: Some(3),
            lang: None,
        };
        let ctx = req.to_eval_context();
        assert_eq!(ctx.vars.get("x"), Some(&5.0));
        assert_eq!(ctx.precision, Some(3));
    }

    // === EvaluateResponse 序列化 ===

    #[test]
    fn test_evaluate_response_scalar() {
        let resp =
            EvaluateResponse::from_eval(EvalResult::Scalar(5.0), "arithmetic".into(), false, None);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""result":5"#));
        assert!(json.contains(r#""domain":"arithmetic""#));
        assert!(json.contains(r#""cache":"miss""#));
    }

    #[test]
    fn test_evaluate_response_cache_hit() {
        let resp =
            EvaluateResponse::from_eval(EvalResult::Scalar(42.0), "arithmetic".into(), true, None);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""cache":"hit""#));
    }

    #[test]
    fn test_evaluate_response_complex() {
        let resp = EvaluateResponse::from_eval(
            EvalResult::Complex(1.0, 2.0),
            "complex".into(),
            false,
            None,
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""re":1.0"#));
        assert!(json.contains(r#""im":2.0"#));
    }

    #[test]
    fn test_evaluate_response_matrix() {
        let resp = EvaluateResponse::from_eval(
            EvalResult::Matrix(vec![vec![1.0, 2.0], vec![3.0, 4.0]]),
            "matrix".into(),
            false,
            None,
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""result":[[1.0,2.0],[3.0,4.0]]"#));
    }

    #[test]
    fn test_evaluate_response_bigint() {
        let resp = EvaluateResponse::from_eval(
            EvalResult::BigInt(BigInt::from(123456789)),
            "number_theory".into(),
            false,
            None,
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""result":"123456789""#));
    }

    #[test]
    fn test_evaluate_response_bigrational() {
        let r = BigRational::new(BigInt::from(1), BigInt::from(3));
        let resp = EvaluateResponse::from_eval(
            EvalResult::BigRational(r),
            "precision".into(),
            false,
            None,
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""num":"1""#));
        assert!(json.contains(r#""den":"3""#));
    }

    #[test]
    fn test_evaluate_response_vector() {
        let resp = EvaluateResponse::from_eval(
            EvalResult::Vector(vec![1.0, 2.0, 3.0]),
            "vector".into(),
            false,
            None,
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""result":[1.0,2.0,3.0]"#));
    }

    #[test]
    fn test_evaluate_response_symbolic() {
        let resp = EvaluateResponse::from_eval(
            EvalResult::Symbolic("2*x".into()),
            "symbolic".into(),
            false,
            None,
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""result":"2*x""#));
    }

    #[test]
    fn test_evaluate_response_steps() {
        let resp = EvaluateResponse::from_eval(
            EvalResult::Steps(vec!["2+9=11".into(), "11*7=77".into()]),
            "arithmetic".into(),
            false,
            None,
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""result":["2+9=11","11*7=77"]"#));
    }

    // === eval_result_to_json 边界情况 ===

    #[test]
    fn test_eval_result_nan_to_string() {
        let json = eval_result_to_json(&EvalResult::Scalar(f64::NAN));
        assert_eq!(json, serde_json::Value::from("NaN"));
    }

    #[test]
    fn test_eval_result_infinity_to_string() {
        let json = eval_result_to_json(&EvalResult::Scalar(f64::INFINITY));
        assert_eq!(json, serde_json::Value::from("inf"));
    }

    #[test]
    fn test_eval_result_complex_list() {
        let json = eval_result_to_json(&EvalResult::ComplexList(vec![(1.0, 2.0), (3.0, -4.0)]));
        let s = serde_json::to_string(&json).unwrap();
        assert!(s.contains(r#""re":1.0"#));
        assert!(s.contains(r#""im":2.0"#));
        assert!(s.contains(r#""re":3.0"#));
        assert!(s.contains(r#""im":-4.0"#));
    }

    #[test]
    fn test_eval_result_latex() {
        let json = eval_result_to_json(&EvalResult::LaTeX(r"\frac{1}{2}".into()));
        assert_eq!(json, serde_json::Value::from(r"\frac{1}{2}"));
    }

    #[test]
    fn test_eval_result_polynomial() {
        let json = eval_result_to_json(&EvalResult::Polynomial(vec![1.0, 0.0, 2.0]));
        let s = serde_json::to_string(&json).unwrap();
        assert_eq!(s, "[1.0,0.0,2.0]");
    }

    // === ServerError ===

    #[test]
    fn test_server_error_display() {
        let e = ServerError::Http("bind failed".into());
        assert_eq!(e.to_string(), "HTTP server error: bind failed");
        let e = ServerError::Mcp("stdio closed".into());
        assert_eq!(e.to_string(), "MCP server error: stdio closed");
    }

    // === validate() 安全约束 ===

    #[test]
    fn test_validate_accepts_valid_request() {
        let req = EvaluateRequest {
            expr: "2+3".into(),
            vars: HashMap::new(),
            precision: None,
            lang: None,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_validate_accepts_max_precision() {
        let req = EvaluateRequest {
            expr: "1/3".into(),
            vars: HashMap::new(),
            precision: Some(MAX_PRECISION),
            lang: None,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_validate_rejects_oversized_precision() {
        let req = EvaluateRequest {
            expr: "1/3".into(),
            vars: HashMap::new(),
            precision: Some(MAX_PRECISION + 1),
            lang: None,
        };
        let err = req.validate().unwrap_err();
        // 422 VALIDATION_ERROR，field="precision"（契约）
        assert!(matches!(err, ApiError::ValidationError { .. }));
    }

    #[test]
    fn test_validate_rejects_oversized_vars() {
        let mut vars = HashMap::new();
        for i in 0..=MAX_VARS {
            vars.insert(format!("v{}", i), 0.0);
        }
        let req = EvaluateRequest {
            expr: "v1".into(),
            vars,
            precision: None,
            lang: None,
        };
        let err = req.validate().unwrap_err();
        // 422 VALIDATION_ERROR，field="vars"（契约）
        assert!(matches!(err, ApiError::ValidationError { .. }));
    }

    // 直接构造 struct 验证 validate() 自身有限性校验（不依赖反序列化层拦截）
    #[test]
    fn test_validate_rejects_non_finite_vars() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut vars = HashMap::new();
            vars.insert("x".to_string(), bad);
            let req = EvaluateRequest {
                expr: "x".into(),
                vars,
                precision: None,
                lang: None,
            };
            let err = req.validate().unwrap_err();
            // 422 VALIDATION_ERROR，field="vars"（输入值不合法）
            assert!(
                matches!(err, ApiError::ValidationError { .. }),
                "NaN/Infinity vars should be rejected"
            );
        }
    }

    // === expr 字段校验 ===
    // 空串/超长/null 字节均为 DoS / 注入向量。

    #[test]
    fn test_validate_rejects_empty_expr() {
        let req = EvaluateRequest {
            expr: String::new(),
            vars: HashMap::new(),
            precision: None,
            lang: None,
        };
        let err = req.validate().unwrap_err();
        assert!(
            matches!(err, ApiError::ValidationError { .. }),
            "empty expr should be rejected"
        );
    }

    #[test]
    fn test_validate_rejects_oversized_expr() {
        let req = EvaluateRequest {
            expr: "x".repeat(MAX_EXPR_LEN + 1),
            vars: HashMap::new(),
            precision: None,
            lang: None,
        };
        let err = req.validate().unwrap_err();
        assert!(
            matches!(err, ApiError::ValidationError { .. }),
            "oversized expr should be rejected"
        );
    }

    #[test]
    fn test_validate_rejects_null_bytes() {
        let req = EvaluateRequest {
            expr: "1+\0+2".to_string(),
            vars: HashMap::new(),
            precision: None,
            lang: None,
        };
        let err = req.validate().unwrap_err();
        assert!(
            matches!(err, ApiError::ValidationError { .. }),
            "expr with null bytes should be rejected"
        );
    }

    // === vars 键名长度 / 内容校验 ===
    // 超长或含 null 字节的键名为内存攻击 / 注入向量。

    #[test]
    fn test_validate_rejects_oversized_var_name() {
        let mut vars = HashMap::new();
        vars.insert("x".repeat(MAX_VAR_NAME_LEN + 1), 1.0);
        let req = EvaluateRequest {
            expr: "x".into(),
            vars,
            precision: None,
            lang: None,
        };
        let err = req.validate().unwrap_err();
        assert!(
            matches!(err, ApiError::ValidationError { .. }),
            "oversized var name should be rejected"
        );
    }

    #[test]
    fn test_validate_accepts_max_var_name_len() {
        let mut vars = HashMap::new();
        vars.insert("x".repeat(MAX_VAR_NAME_LEN), 1.0);
        let req = EvaluateRequest {
            expr: "x".into(),
            vars,
            precision: None,
            lang: None,
        };
        assert!(req.validate().is_ok(), "max var name length should pass");
    }

    #[test]
    fn test_validate_rejects_null_bytes_in_var_name() {
        let mut vars = HashMap::new();
        vars.insert("x\0y".to_string(), 1.0);
        let req = EvaluateRequest {
            expr: "x".into(),
            vars,
            precision: None,
            lang: None,
        };
        let err = req.validate().unwrap_err();
        assert!(
            matches!(err, ApiError::ValidationError { .. }),
            "var name with null bytes should be rejected"
        );
    }
}
