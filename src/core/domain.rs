// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 计算域接口与路由器：CalculationDomain trait + 按优先级降序路由的 DomainRouter。

use crate::core::types::{AstNode, CalcError, EvalContext, EvalResult};
use std::collections::HashSet;

/// 计算域接口：声明域名称、supports()、evaluate()、优先级与非确定性函数列表。
pub trait CalculationDomain: Send + Sync {
    /// 域名称（如 `"arithmetic"`、`"scientific"`）。
    fn domain_name(&self) -> &str;

    /// 是否支持处理该 AST（路由器按优先级降序遍历，首个 true 胜出）。
    fn supports(&self, ast: &AstNode) -> bool;

    /// 求值 AST（调用前应先用 `supports()` 确认支持）。
    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError>;

    /// 优先级（数值越大优先级越高，同优先级按注册顺序）。
    fn priority(&self) -> u8;

    /// 申报本域的非确定性函数名列表（默认空）。
    fn nondeterministic_functions(&self) -> &'static [&'static str] {
        &[]
    }
}

/// 域路由器：按优先级降序选首个 `supports()` 为真的域（Send+Sync，注册时聚合非确定性函数名）。
pub struct DomainRouter {
    domains: Vec<Box<dyn CalculationDomain>>,
    /// 聚合的非确定性函数名（register 时增量构建）
    nondeterministic_functions: HashSet<String>,
}

impl DomainRouter {
    /// 创建空路由器。
    pub fn new() -> Self {
        Self {
            domains: Vec::new(),
            nondeterministic_functions: HashSet::new(),
        }
    }

    /// 注册计算域。
    ///
    /// 注册后按 `priority()` 降序稳定排序，并聚合其非确定性函数名；同名域重复注册时忽略。
    pub fn register(&mut self, domain: Box<dyn CalculationDomain>) {
        let name = domain.domain_name().to_string();
        if self.domains.iter().any(|d| d.domain_name() == name) {
            return;
        }
        for &func_name in domain.nondeterministic_functions() {
            self.nondeterministic_functions
                .insert(func_name.to_string());
        }
        self.domains.push(domain);
        self.domains
            .sort_by_key(|d| std::cmp::Reverse(d.priority()));
    }

    /// 路由 AST 到第一个支持的域；无域支持时返回 DomainError。
    pub fn route(&self, ast: &AstNode) -> Result<&dyn CalculationDomain, CalcError> {
        for domain in &self.domains {
            if domain.supports(ast) {
                return Ok(domain.as_ref());
            }
        }
        let functions = collect_function_names(ast);
        let detail = if functions.is_empty() {
            "no functions".to_string()
        } else {
            // 限制函数名显示数量，防止无界长度错误消息污染日志与 UI。
            format_functions_list(&functions)
        };
        Err(CalcError::domain(format!(
            "no registered domain supports this expression ({})",
            detail
        ))
        .with_i18n(
            "msg.core.no_registered_domain",
            vec![("expr".to_string(), detail.to_string())],
        ))
    }

    /// 已注册域数量。
    pub fn domain_count(&self) -> usize {
        self.domains.len()
    }

    /// 获取所有已注册域的名称（按优先级降序）。
    pub fn domain_names(&self) -> Vec<&str> {
        self.domains.iter().map(|d| d.domain_name()).collect()
    }

    /// 检测 AST 是否含非确定性函数调用，供 evaluator 旁路缓存读写（如 now()/fx()）。
    pub fn is_nondeterministic(&self, ast: &AstNode) -> bool {
        let functions = collect_function_names(ast);
        functions
            .iter()
            .any(|f| self.nondeterministic_functions.contains(f))
    }
}

impl Default for DomainRouter {
    fn default() -> Self {
        Self::new()
    }
}

// 编译期 Send + Sync 检查（coverage 运行时排除：const fn 在编译期执行，无法被行覆盖）
#[cfg(not(coverage))]
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DomainRouter>();
};

/// 递归收集 AST 中的所有函数名（用于错误信息）。
fn collect_function_names(ast: &AstNode) -> Vec<String> {
    let mut names = Vec::new();
    collect_function_names_recursive(ast, &mut names);
    names.sort();
    names.dedup();
    names
}

/// 错误消息中最多显示的函数名数量，超出截断并追加 `... and N more`。
const MAX_DISPLAY_FUNCTIONS: usize = 5;

/// 格式化函数名列表用于错误消息，超长时截断。
fn format_functions_list(functions: &[String]) -> String {
    if functions.len() <= MAX_DISPLAY_FUNCTIONS {
        return format!("functions: {:?}", functions);
    }
    // 截断：显示前 MAX_DISPLAY_FUNCTIONS 个，追加 "... and N more"
    let displayed: Vec<&str> = functions
        .iter()
        .take(MAX_DISPLAY_FUNCTIONS)
        .map(String::as_str)
        .collect();
    let remaining = functions.len() - MAX_DISPLAY_FUNCTIONS;
    format!("functions: {:?}, ... and {} more", displayed, remaining)
}

fn collect_function_names_recursive(ast: &AstNode, names: &mut Vec<String>) {
    match ast {
        AstNode::FunctionCall(name, args) => {
            names.push(name.clone());
            for arg in args {
                collect_function_names_recursive(arg, names);
            }
        }
        AstNode::BinaryOp(_, l, r) => {
            collect_function_names_recursive(l, names);
            collect_function_names_recursive(r, names);
        }
        AstNode::UnaryOp(_, e) => {
            collect_function_names_recursive(e, names);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ErrorKind;
    use crate::core::parser::parse;

    /// 科学函数集合。
    const SCIENTIFIC_FUNCTIONS: &[&str] = &[
        "sin", "cos", "tan", "asin", "acos", "atan", "ln", "log", "exp", "sinh", "cosh", "tanh",
        "gamma", "erf",
    ];

    /// 算术函数集合（`!`→`factorial`、`%`→`mod`、`abs`）。
    const ARITHMETIC_FUNCTIONS: &[&str] = &["factorial", "mod", "abs"];

    /// 递归检查 AST 是否包含科学函数调用。
    fn contains_scientific_function(ast: &AstNode) -> bool {
        match ast {
            AstNode::FunctionCall(name, args) => {
                SCIENTIFIC_FUNCTIONS.contains(&name.as_str())
                    || args.iter().any(contains_scientific_function)
            }
            AstNode::BinaryOp(_, l, r) => {
                contains_scientific_function(l) || contains_scientific_function(r)
            }
            AstNode::UnaryOp(_, e) => contains_scientific_function(e),
            _ => false,
        }
    }

    /// 检查 AST 是否仅包含算术运算（无科学函数、无未知函数）。
    fn is_arithmetic_only(ast: &AstNode) -> bool {
        match ast {
            AstNode::Number(_) | AstNode::Variable(_) => true,
            AstNode::BinaryOp(_, l, r) => is_arithmetic_only(l) && is_arithmetic_only(r),
            AstNode::UnaryOp(_, e) => is_arithmetic_only(e),
            AstNode::FunctionCall(name, args) => {
                ARITHMETIC_FUNCTIONS.contains(&name.as_str()) && args.iter().all(is_arithmetic_only)
            }
            AstNode::Complex(_, _)
            | AstNode::Matrix(_)
            | AstNode::List(_)
            | AstNode::BigNumber(_)
            | AstNode::Str(_) => false,
        }
    }

    /// Mock Arithmetic 域：仅支持算术表达式。
    struct MockArithmeticDomain;

    impl CalculationDomain for MockArithmeticDomain {
        fn domain_name(&self) -> &str {
            "arithmetic"
        }
        fn priority(&self) -> u8 {
            10
        }
        fn supports(&self, ast: &AstNode) -> bool {
            is_arithmetic_only(ast)
        }
        fn evaluate(&self, _ast: &AstNode, _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
            Ok(EvalResult::Scalar(0.0))
        }
    }

    /// Mock Scientific 域：支持含科学函数的表达式。
    struct MockScientificDomain;

    impl CalculationDomain for MockScientificDomain {
        fn domain_name(&self) -> &str {
            "scientific"
        }
        fn priority(&self) -> u8 {
            20
        }
        fn supports(&self, ast: &AstNode) -> bool {
            contains_scientific_function(ast)
        }
        fn evaluate(&self, _ast: &AstNode, _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
            Ok(EvalResult::Scalar(0.0))
        }
    }

    /// 可配置 Mock 域（用于优先级测试）。
    struct ConfigurableMockDomain {
        name: String,
        priority: u8,
        supports_result: bool,
    }

    impl CalculationDomain for ConfigurableMockDomain {
        fn domain_name(&self) -> &str {
            &self.name
        }
        fn priority(&self) -> u8 {
            self.priority
        }
        fn supports(&self, _ast: &AstNode) -> bool {
            self.supports_result
        }
        fn evaluate(&self, _ast: &AstNode, _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
            Ok(EvalResult::Scalar(0.0))
        }
    }

    /// 创建注册了 Arithmetic + Scientific 的默认路由器。
    fn default_router() -> DomainRouter {
        let mut router = DomainRouter::new();
        router.register(Box::new(MockArithmeticDomain));
        router.register(Box::new(MockScientificDomain));
        router
    }

    // ===== 1: 路由至 Arithmetic 计算域 =====

    #[test]
    fn test_route_arithmetic_basic() {
        // (2+9)*7-6 → Arithmetic
        let router = default_router();
        let ast = parse("(2+9)*7-6").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "arithmetic");
    }

    #[test]
    fn test_route_arithmetic_power_factorial() {
        // 2^10 + 5! → Arithmetic
        let router = default_router();
        let ast = parse("2^10 + factorial(5)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "arithmetic");
    }

    #[test]
    fn test_route_arithmetic_mod_abs() {
        // 10%3 + abs(-2) → Arithmetic
        let router = default_router();
        let ast = parse("mod(10,3) + abs(-2)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "arithmetic");
    }

    #[test]
    fn test_route_arithmetic_constant() {
        // 42 → Arithmetic（默认域）
        let router = default_router();
        let ast = parse("42").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "arithmetic");
    }

    // ===== 2: 路由至 Scientific 计算域 =====

    #[test]
    fn test_route_scientific_trig() {
        // sin(pi/2) → Scientific
        let router = default_router();
        let ast = parse("sin(pi/2)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "scientific");
    }

    #[test]
    fn test_route_scientific_log() {
        // log(100, 10) → Scientific
        let router = default_router();
        let ast = parse("log(100, 10)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "scientific");
    }

    #[test]
    fn test_route_scientific_gamma_erf() {
        // gamma(5) + erf(1) → Scientific
        let router = default_router();
        let ast = parse("gamma(5) + erf(1)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "scientific");
    }

    #[test]
    fn test_route_scientific_hyperbolic() {
        // sinh(1) + cosh(1) → Scientific
        let router = default_router();
        let ast = parse("sinh(1) + cosh(1)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "scientific");
    }

    // ===== 3: 混合表达式路由优先级 =====

    #[test]
    fn test_mixed_expression_routes_to_scientific() {
        // sin(x) + 2*3 → Scientific
        let router = default_router();
        let ast = parse("sin(x) + 2*3").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "scientific");
    }

    #[test]
    fn test_scientific_priority_higher_than_arithmetic() {
        let arithmetic = MockArithmeticDomain;
        let scientific = MockScientificDomain;
        assert!(scientific.priority() > arithmetic.priority());
    }

    #[test]
    fn test_pure_arithmetic_not_supported_by_scientific() {
        // 2+3*4 → Scientific.supports = false
        let scientific = MockScientificDomain;
        let ast = parse("2+3*4").unwrap();
        assert!(!scientific.supports(&ast));

        let router = default_router();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "arithmetic");
    }

    // ===== 4: 多域优先级匹配 =====

    #[test]
    fn test_higher_priority_wins() {
        // Domain B (priority=20) wins over Domain A (priority=10)
        let mut router = DomainRouter::new();
        router.register(Box::new(ConfigurableMockDomain {
            name: "A".to_string(),
            priority: 10,
            supports_result: true,
        }));
        router.register(Box::new(ConfigurableMockDomain {
            name: "B".to_string(),
            priority: 20,
            supports_result: true,
        }));

        let ast = parse("1").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "B");
    }

    #[test]
    fn test_same_priority_registration_order_wins() {
        // Same priority → first registered wins
        let mut router = DomainRouter::new();
        router.register(Box::new(ConfigurableMockDomain {
            name: "first".to_string(),
            priority: 10,
            supports_result: true,
        }));
        router.register(Box::new(ConfigurableMockDomain {
            name: "second".to_string(),
            priority: 10,
            supports_result: true,
        }));

        let ast = parse("1").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "first");
    }

    #[test]
    fn test_only_one_supports_selected() {
        // Only one domain supports → selected regardless of priority
        let mut router = DomainRouter::new();
        router.register(Box::new(ConfigurableMockDomain {
            name: "high_not_supporting".to_string(),
            priority: 100,
            supports_result: false,
        }));
        router.register(Box::new(ConfigurableMockDomain {
            name: "low_supporting".to_string(),
            priority: 1,
            supports_result: true,
        }));

        let ast = parse("1").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "low_supporting");
    }

    // ===== 5: 无匹配域返回错误 =====

    #[test]
    fn test_unknown_function_no_match() {
        // foo(1) → no domain supports
        let router = default_router();
        let ast = parse("foo(1)").unwrap();
        let result = router.route(&ast);
        let e = result.err().expect("expected error");
        assert!(
            e.kind == ErrorKind::Domain,
            "expected DomainError, got {:?}",
            e
        );
    }

    #[test]
    fn test_no_domains_registered_error() {
        // No domains registered → error
        let router = DomainRouter::new();
        let ast = parse("1+2").unwrap();
        let result = router.route(&ast);
        let e = result.err().expect("expected error");
        assert!(
            e.kind == ErrorKind::Domain,
            "expected DomainError, got {:?}",
            e
        );
    }

    #[test]
    fn test_error_message_contains_function_name() {
        // bar(2) → error message contains "bar"
        let router = default_router();
        let ast = parse("bar(2)").unwrap();
        let err = router.route(&ast).err().expect("expected error");
        assert_eq!(
            err.kind,
            ErrorKind::Domain,
            "expected DomainError, got {:?}",
            err
        );
        let msg = &err.message;
        assert!(
            msg.contains("bar"),
            "error message should contain 'bar': {}",
            msg
        );
    }

    // ===== 6: 计算域注册机制 =====

    #[test]
    fn test_router_loads_registered_domains() {
        let router = default_router();
        assert_eq!(router.domain_count(), 2);
        let names = router.domain_names();
        assert!(names.contains(&"arithmetic"));
        assert!(names.contains(&"scientific"));
    }

    #[test]
    fn test_register_then_route() {
        // Register new domain → can route to it
        let mut router = DomainRouter::new();
        router.register(Box::new(ConfigurableMockDomain {
            name: "custom".to_string(),
            priority: 50,
            supports_result: true,
        }));

        let ast = parse("1").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "custom");
    }

    #[test]
    fn test_domains_sorted_by_priority_desc() {
        // Domains sorted by priority descending
        let mut router = DomainRouter::new();
        router.register(Box::new(MockArithmeticDomain)); // priority=10
        router.register(Box::new(MockScientificDomain)); // priority=20

        // Scientific (20) should come before Arithmetic (10)
        let names = router.domain_names();
        assert_eq!(names, vec!["scientific", "arithmetic"]);
    }

    #[test]
    fn test_unimplemented_trait_cannot_register() {
        // 编译期检查：register 只接受 Box<dyn CalculationDomain>（取消注释下方代码将编译失败）
        // router.register(Box::new(42_i32));
        let mut router = DomainRouter::new();
        router.register(Box::new(MockArithmeticDomain));
        assert_eq!(router.domain_count(), 1);
    }

    // ===== DomainRouter Send + Sync =====

    #[test]
    fn test_router_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        let router = DomainRouter::new();
        assert_send_sync(&router);
    }

    // ===== 覆盖 Default impl 与 Mock evaluate 方法 =====

    #[test]
    fn test_domain_router_default() {
        // 覆盖 impl Default for DomainRouter（lines 98-100）
        let router = DomainRouter::default();
        assert_eq!(router.domain_count(), 0);
    }

    #[test]
    fn test_mock_arithmetic_domain_evaluate() {
        // 覆盖 MockArithmeticDomain::evaluate（lines 190-192）
        let domain = MockArithmeticDomain;
        let ast = parse("1+2").unwrap();
        let ctx = EvalContext::new();
        let result = domain.evaluate(&ast, &ctx).unwrap();
        assert_eq!(result, EvalResult::Scalar(0.0));
    }

    #[test]
    fn test_mock_scientific_domain_evaluate() {
        // 覆盖 MockScientificDomain::evaluate（lines 204-206）
        let domain = MockScientificDomain;
        let ast = parse("sin(1)").unwrap();
        let ctx = EvalContext::new();
        let result = domain.evaluate(&ast, &ctx).unwrap();
        assert_eq!(result, EvalResult::Scalar(0.0));
    }

    #[test]
    fn test_configurable_mock_domain_evaluate() {
        // 覆盖 ConfigurableMockDomain::evaluate（lines 220-222）
        let domain = ConfigurableMockDomain {
            name: "test".to_string(),
            priority: 10,
            supports_result: true,
        };
        let ast = parse("1").unwrap();
        let ctx = EvalContext::new();
        let result = domain.evaluate(&ast, &ctx).unwrap();
        assert_eq!(result, EvalResult::Scalar(0.0));
    }

    #[test]
    fn test_is_arithmetic_only_non_arithmetic_nodes() {
        // 覆盖 is_arithmetic_only 中 Complex/Matrix/List/BigNumber → false 分支
        assert!(!is_arithmetic_only(&AstNode::Complex(1.0, 2.0)));
        assert!(!is_arithmetic_only(&AstNode::Matrix(vec![vec![
            AstNode::Number(1.0)
        ]])));
        assert!(!is_arithmetic_only(&AstNode::List(vec![AstNode::Number(
            1.0
        )])));
        assert!(!is_arithmetic_only(&AstNode::BigNumber("123".to_string())));
    }

    #[test]
    fn test_collect_function_names_unary_op_branch() {
        // 覆盖 collect_function_names_recursive 的 UnaryOp 分支（lines 130-132）
        // -foo(1) → 路由失败时递归收集函数名，应处理 UnaryOp 节点
        let router = default_router();
        let ast = AstNode::UnaryOp(
            crate::core::types::UnaryOp::Neg,
            Box::new(AstNode::FunctionCall(
                "foo".to_string(),
                vec![AstNode::Number(1.0)],
            )),
        );
        let err = router.route(&ast).err().expect("expected error");
        assert!(
            err.to_string().contains("foo"),
            "错误信息应包含 'foo': {}",
            err
        );
    }

    // ===== 回归测试：长函数列表必须截断 =====
    // 错误消息仅显示前 5 个函数名，超出显示 "... and N more"（防无界长度消息）。
    #[test]
    fn test_route_error_truncates_long_function_list() {
        // 构造含 50 个未知函数的 AST，错误消息应截断而非全量展示
        let mut ast =
            AstNode::FunctionCall("unknown_func_00".to_string(), vec![AstNode::Number(1.0)]);
        for i in 1..50 {
            ast = AstNode::BinaryOp(
                crate::core::types::BinaryOp::Add,
                Box::new(ast),
                Box::new(AstNode::FunctionCall(
                    format!("unknown_func_{:02}", i),
                    vec![AstNode::Number(1.0)],
                )),
            );
        }

        let router = default_router();
        let err = router.route(&ast).err().expect("expected error");
        let msg = err.to_string();

        // 截断阈值：错误消息应远小于全量展示的长度
        // 全量展示 50 个函数 × ~20 字符 = ~1000 字符（不含前缀）
        // 截断后应 < 300 字符（含前缀和截断提示）
        assert!(
            msg.len() < 500,
            "error message too long ({} chars), should be truncated: {}",
            msg.len(),
            msg
        );

        // 截断提示：应包含明确的截断指示符
        assert!(
            msg.contains("more") || msg.contains("..."),
            "error message should indicate truncation: {}",
            msg
        );
    }

    #[test]
    fn test_route_error_short_function_list_not_truncated() {
        // 短函数列表（≤ 阈值）不应截断，保持原有行为
        let ast = AstNode::FunctionCall("unknown_func".to_string(), vec![AstNode::Number(1.0)]);
        let router = default_router();
        let err = router.route(&ast).err().expect("expected error");
        let msg = err.to_string();

        // 单个函数不应触发截断
        assert!(
            msg.contains("unknown_func") && !msg.contains("more"),
            "short list should not be truncated: {}",
            msg
        );
    }

    // ===== proptest 属性测试 =====

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        // 路由确定性：同一 AST 多次路由结果相同
        #[test]
        fn prop_routing_deterministic(expr in (1u32..1000).prop_map(|n| format!("{}+{}", n, n+1))) {
            let router = default_router();
            let ast = parse(&expr).unwrap();
            let d1 = router.route(&ast).unwrap().domain_name();
            let d2 = router.route(&ast).unwrap().domain_name();
            prop_assert_eq!(d1, d2);
        }

        // 优先级一致性：高优先级域始终优先
        #[test]
        fn prop_priority_consistent(
            high_prio in 50u8..255,
            low_prio in 0u8..49
        ) {
            let mut router = DomainRouter::new();
            router.register(Box::new(ConfigurableMockDomain {
                name: "low".to_string(),
                priority: low_prio,
                supports_result: true,
            }));
            router.register(Box::new(ConfigurableMockDomain {
                name: "high".to_string(),
                priority: high_prio,
                supports_result: true,
            }));
            let ast = parse("1").unwrap();
            let domain = router.route(&ast).unwrap();
            prop_assert_eq!(domain.domain_name(), "high");
        }
    }

    // ===== 新增域路由测试 =====
    // 统一用 build_default_router()，core 测试不引用具体域类型（priority 测试在 domains/factory.rs）。

    // ----- 6.1 验证 collect_function_names_recursive 已支持 FunctionCall 递归 -----
    // 已在 test_collect_function_names_unary_op_branch 和既有测试中覆盖，此处不再重复。

    // ----- 6.2 路由测试：4 新域 -----

    #[test]
    fn test_v08_route_number_theory() {
        let router = crate::domains::build_default_router();
        let ast = parse("gcd(12,18)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "number_theory");
    }

    #[test]
    fn test_v08_route_combinatorics() {
        let router = crate::domains::build_default_router();
        let ast = parse("P(5,2)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "combinatorics");
    }

    #[test]
    fn test_v08_route_vector() {
        let router = crate::domains::build_default_router();
        let ast = parse("dot([1,2],[3,4])").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "vector");
    }

    #[test]
    fn test_v08_route_polynomial() {
        let router = crate::domains::build_default_router();
        let ast = parse("poly_add(x+1,x-1)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "polynomial");
    }

    // ----- 6.3 优先级测试 -----
    // priority 测试在 src/domains/factory.rs（core → domains 类型依赖 = 0）。

    // ----- 6.4 消歧测试 -----

    #[test]
    fn test_v08_disambiguate_mod_pow_to_number_theory() {
        // mod_pow 路由至 NumberTheory 而非 Arithmetic
        let router = crate::domains::build_default_router();
        let ast = parse("mod_pow(2,10,1000)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "number_theory");
    }

    #[test]
    fn test_v08_disambiguate_mean_to_statistics() {
        // mean([1,2,3]) 路由至 Statistics 而非 Vector
        let router = crate::domains::build_default_router();
        let ast = parse("mean([1,2,3])").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "statistics");
    }

    // ----- 6.5 无匹配域错误测试 -----

    #[test]
    fn test_v08_unknown_function_error() {
        // unknown_func(1) → DomainError 且消息含函数名
        let router = crate::domains::build_default_router();
        let ast = parse("unknown_func(1)").unwrap();
        let result = router.route(&ast);
        let e = result.err().expect("expected error");
        assert!(e.kind == ErrorKind::Domain);
        assert!(
            e.to_string().contains("unknown_func"),
            "错误信息应包含 'unknown_func': {}",
            e
        );
    }

    // ----- 6.6 非确定性函数检测测试 -----

    /// 申报非确定性函数的 Mock 域（用于 is_nondeterministic 测试）。
    struct NondeterministicMockDomain {
        funcs: &'static [&'static str],
    }

    impl CalculationDomain for NondeterministicMockDomain {
        fn domain_name(&self) -> &str {
            "nondeterministic_mock"
        }
        fn priority(&self) -> u8 {
            50
        }
        fn supports(&self, _ast: &AstNode) -> bool {
            false
        }
        fn evaluate(&self, _ast: &AstNode, _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
            Ok(EvalResult::Scalar(0.0))
        }
        fn nondeterministic_functions(&self) -> &'static [&'static str] {
            self.funcs
        }
    }

    #[test]
    fn test_is_nondeterministic_empty_router() {
        // 空申报路由器恒 false（无任何申报时恒 false）
        let router = DomainRouter::new();
        let ast = parse("now()").unwrap();
        assert!(!router.is_nondeterministic(&ast));
        let ast = parse("sin(1)").unwrap();
        assert!(!router.is_nondeterministic(&ast));
    }

    #[test]
    fn test_is_nondeterministic_with_declaration() {
        // 申报 ["now"] 后：now() → true、now()+1 → true、sin(now()) → true、sin(1) → false
        let mut router = DomainRouter::new();
        router.register(Box::new(NondeterministicMockDomain { funcs: &["now"] }));

        assert!(
            router.is_nondeterministic(&parse("now()").unwrap()),
            "now() 必须检测为非确定性"
        );
        assert!(
            router.is_nondeterministic(&parse("now()+1").unwrap()),
            "now()+1 必须检测为非确定性"
        );
        assert!(
            router.is_nondeterministic(&parse("sin(now())").unwrap()),
            "sin(now()) 必须检测为非确定性"
        );
        assert!(
            !router.is_nondeterministic(&parse("sin(1)").unwrap()),
            "sin(1) 不含非确定性函数"
        );
        assert!(
            !router.is_nondeterministic(&parse("1+2").unwrap()),
            "1+2 不含非确定性函数"
        );
    }

    #[test]
    fn test_is_nondeterministic_multiple_functions() {
        // 多函数申报：任一命中即 true
        let mut router = DomainRouter::new();
        router.register(Box::new(NondeterministicMockDomain {
            funcs: &["now", "today", "fx"],
        }));

        assert!(router.is_nondeterministic(&parse("now()").unwrap()));
        assert!(router.is_nondeterministic(&parse("today()").unwrap()));
        assert!(router.is_nondeterministic(&parse("fx(1,\"USD\",\"CNY\")").unwrap()));
        assert!(!router.is_nondeterministic(&parse("sin(1)").unwrap()));
    }
}
