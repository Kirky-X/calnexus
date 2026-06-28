//! 计算域接口与路由器。
//!
//! 设计依据：
//! - design.md D4：trait-kit 作为域接口（fallback：自研 trait + 手动注册表）
//! - domain-routing spec：6 个 requirements / 21 个 scenarios
//!
//! 核心类型：
//! - [`CalculationDomain`]：所有计算域必须实现的 trait
//! - [`DomainRouter`]：按优先级降序遍历域，选择第一个 `supports()` 返回 true 的域
//!
//! **设计偏差**：design.md D4 原计划使用 trait-kit `ModuleInterface`，
//! 但 trait-kit API 不明确且 crates.io 下载量为 0。采用设计文档中预批准的 fallback
//! 方案（"自研 trait + 手动注册表"），功能等价且更可控。

use crate::core::types::{AstNode, CalcError, EvalContext, EvalResult};

/// 计算域接口：所有计算域必须实现此 trait。
///
/// 每个域需声明：
/// - 域名称（用于路由结果标识）
/// - `supports()` 判断是否能处理某 AST
/// - `evaluate()` 执行实际计算
/// - 优先级（数值越大越优先）
pub trait CalculationDomain: Send + Sync {
    /// 域名称（如 `"arithmetic"`、`"scientific"`）。
    fn domain_name(&self) -> &str;

    /// 是否支持处理该 AST。
    ///
    /// 路由器按优先级降序遍历，首个返回 `true` 的域胜出。
    fn supports(&self, ast: &AstNode) -> bool;

    /// 求值 AST。
    ///
    /// 调用前应先通过 `supports()` 确认该域支持此 AST。
    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError>;

    /// 优先级（数值越大优先级越高，同优先级按注册顺序）。
    fn priority(&self) -> u8;
}

/// 域路由器：按优先级降序遍历已注册域，选择第一个 `supports()` 返回 `true` 的域。
///
/// 线程安全（`Send + Sync`），支持并发路由查询。
pub struct DomainRouter {
    domains: Vec<Box<dyn CalculationDomain>>,
}

impl DomainRouter {
    /// 创建空路由器。
    pub fn new() -> Self {
        Self { domains: Vec::new() }
    }

    /// 注册计算域。
    ///
    /// 注册后按 `priority()` 降序稳定排序（同优先级保持注册顺序）。
    pub fn register(&mut self, domain: Box<dyn CalculationDomain>) {
        self.domains.push(domain);
        // 稳定排序：同优先级时保持注册顺序（Req 4 Scen 2）
        self.domains.sort_by(|a, b| b.priority().cmp(&a.priority()));
    }

    /// 路由 AST 到第一个支持的域。
    ///
    /// 按 `priority()` 降序遍历，返回第一个 `supports()` 返回 `true` 的域引用。
    /// 若无域支持，返回 `CalcError::DomainError`，错误信息包含 AST 中的函数名（Req 5 Scen 3）。
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
            format!("functions: {:?}", functions)
        };
        Err(CalcError::DomainError(format!(
            "no registered domain supports this expression ({})",
            detail
        )))
    }

    /// 已注册域数量。
    pub fn domain_count(&self) -> usize {
        self.domains.len()
    }

    /// 获取所有已注册域的名称（按优先级降序）。
    pub fn domain_names(&self) -> Vec<&str> {
        self.domains.iter().map(|d| d.domain_name()).collect()
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

/// 递归收集 AST 中的所有函数名（用于错误信息，Req 5 Scen 3）。
fn collect_function_names(ast: &AstNode) -> Vec<String> {
    let mut names = Vec::new();
    collect_function_names_recursive(ast, &mut names);
    names.sort();
    names.dedup();
    names
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
    use crate::core::parser::parse;

    /// 科学函数集合（Req 2）。
    const SCIENTIFIC_FUNCTIONS: &[&str] = &[
        "sin", "cos", "tan", "asin", "acos", "atan",
        "ln", "log", "exp", "sinh", "cosh", "tanh",
        "gamma", "erf",
    ];

    /// 算术函数集合（Req 1：`!`→`factorial`、`%`→`mod`、`abs`）。
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
                ARITHMETIC_FUNCTIONS.contains(&name.as_str())
                    && args.iter().all(is_arithmetic_only)
            }
            AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) | AstNode::BigNumber(_) => false,
        }
    }

    /// Mock Arithmetic 域：仅支持算术表达式（Req 1）。
    struct MockArithmeticDomain;

    impl CalculationDomain for MockArithmeticDomain {
        fn domain_name(&self) -> &str { "arithmetic" }
        fn priority(&self) -> u8 { 10 }
        fn supports(&self, ast: &AstNode) -> bool {
            is_arithmetic_only(ast)
        }
        fn evaluate(&self, _ast: &AstNode, _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
            Ok(EvalResult::Scalar(0.0))
        }
    }

    /// Mock Scientific 域：支持含科学函数的表达式（Req 2）。
    struct MockScientificDomain;

    impl CalculationDomain for MockScientificDomain {
        fn domain_name(&self) -> &str { "scientific" }
        fn priority(&self) -> u8 { 20 }
        fn supports(&self, ast: &AstNode) -> bool {
            contains_scientific_function(ast)
        }
        fn evaluate(&self, _ast: &AstNode, _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
            Ok(EvalResult::Scalar(0.0))
        }
    }

    /// 可配置 Mock 域（用于优先级测试，Req 4）。
    struct ConfigurableMockDomain {
        name: String,
        priority: u8,
        supports_result: bool,
    }

    impl CalculationDomain for ConfigurableMockDomain {
        fn domain_name(&self) -> &str { &self.name }
        fn priority(&self) -> u8 { self.priority }
        fn supports(&self, _ast: &AstNode) -> bool { self.supports_result }
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

    // ===== Requirement 1: 路由至 Arithmetic 计算域 =====

    #[test]
    fn test_route_arithmetic_basic() {
        // (2+9)*7-6 → Arithmetic (Req 1 Scen 1)
        let router = default_router();
        let ast = parse("(2+9)*7-6").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "arithmetic");
    }

    #[test]
    fn test_route_arithmetic_power_factorial() {
        // 2^10 + 5! → Arithmetic (Req 1 Scen 2)
        let router = default_router();
        let ast = parse("2^10 + factorial(5)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "arithmetic");
    }

    #[test]
    fn test_route_arithmetic_mod_abs() {
        // 10%3 + abs(-2) → Arithmetic (Req 1 Scen 3)
        let router = default_router();
        let ast = parse("mod(10,3) + abs(-2)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "arithmetic");
    }

    #[test]
    fn test_route_arithmetic_constant() {
        // 42 → Arithmetic (默认域, Req 1 Scen 4)
        let router = default_router();
        let ast = parse("42").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "arithmetic");
    }

    // ===== Requirement 2: 路由至 Scientific 计算域 =====

    #[test]
    fn test_route_scientific_trig() {
        // sin(pi/2) → Scientific (Req 2 Scen 1)
        let router = default_router();
        let ast = parse("sin(pi/2)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "scientific");
    }

    #[test]
    fn test_route_scientific_log() {
        // log(100, 10) → Scientific (Req 2 Scen 2)
        let router = default_router();
        let ast = parse("log(100, 10)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "scientific");
    }

    #[test]
    fn test_route_scientific_gamma_erf() {
        // gamma(5) + erf(1) → Scientific (Req 2 Scen 3)
        let router = default_router();
        let ast = parse("gamma(5) + erf(1)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "scientific");
    }

    #[test]
    fn test_route_scientific_hyperbolic() {
        // sinh(1) + cosh(1) → Scientific (Req 2 Scen 4)
        let router = default_router();
        let ast = parse("sinh(1) + cosh(1)").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "scientific");
    }

    // ===== Requirement 3: 混合表达式路由优先级 =====

    #[test]
    fn test_mixed_expression_routes_to_scientific() {
        // sin(x) + 2*3 → Scientific (Req 3 Scen 1)
        let router = default_router();
        let ast = parse("sin(x) + 2*3").unwrap();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "scientific");
    }

    #[test]
    fn test_scientific_priority_higher_than_arithmetic() {
        // Scientific priority > Arithmetic priority (Req 3 Scen 2)
        let arithmetic = MockArithmeticDomain;
        let scientific = MockScientificDomain;
        assert!(scientific.priority() > arithmetic.priority());
    }

    #[test]
    fn test_pure_arithmetic_not_supported_by_scientific() {
        // 2+3*4 → Scientific.supports = false (Req 3 Scen 3)
        let scientific = MockScientificDomain;
        let ast = parse("2+3*4").unwrap();
        assert!(!scientific.supports(&ast));

        let router = default_router();
        let domain = router.route(&ast).unwrap();
        assert_eq!(domain.domain_name(), "arithmetic");
    }

    // ===== Requirement 4: 多域优先级匹配 =====

    #[test]
    fn test_higher_priority_wins() {
        // Domain B (priority=20) wins over Domain A (priority=10) (Req 4 Scen 1)
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
        // Same priority → first registered wins (Req 4 Scen 2)
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
        // Only one domain supports → selected regardless of priority (Req 4 Scen 3)
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

    // ===== Requirement 5: 无匹配域返回错误 =====

    #[test]
    fn test_unknown_function_no_match() {
        // foo(1) → no domain supports (Req 5 Scen 1)
        let router = default_router();
        let ast = parse("foo(1)").unwrap();
        let result = router.route(&ast);
        let e = result.err().expect("expected error");
        assert!(matches!(e, CalcError::DomainError(_)), "expected DomainError, got {:?}", e);
    }

    #[test]
    fn test_no_domains_registered_error() {
        // No domains registered → error (Req 5 Scen 2)
        let router = DomainRouter::new();
        let ast = parse("1+2").unwrap();
        let result = router.route(&ast);
        let e = result.err().expect("expected error");
        assert!(matches!(e, CalcError::DomainError(_)), "expected DomainError, got {:?}", e);
    }

    #[test]
    fn test_error_message_contains_function_name() {
        // bar(2) → error message contains "bar" (Req 5 Scen 3)
        let router = default_router();
        let ast = parse("bar(2)").unwrap();
        let err = router.route(&ast).err().expect("expected error");
        let CalcError::DomainError(msg) = err else { panic!("expected DomainError, got {:?}", err) };
        assert!(msg.contains("bar"), "error message should contain 'bar': {}", msg);
    }

    // ===== Requirement 6: 计算域注册机制 =====

    #[test]
    fn test_router_loads_registered_domains() {
        // Router loads registered domains (Req 6 Scen 1)
        let router = default_router();
        assert_eq!(router.domain_count(), 2);
        let names = router.domain_names();
        assert!(names.contains(&"arithmetic"));
        assert!(names.contains(&"scientific"));
    }

    #[test]
    fn test_register_then_route() {
        // Register new domain → can route to it (Req 6 Scen 2)
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
        // Domains sorted by priority descending (Req 6 Scen 3)
        let mut router = DomainRouter::new();
        router.register(Box::new(MockArithmeticDomain)); // priority=10
        router.register(Box::new(MockScientificDomain)); // priority=20

        // Scientific (20) should come before Arithmetic (10)
        let names = router.domain_names();
        assert_eq!(names, vec!["scientific", "arithmetic"]);
    }

    #[test]
    fn test_unimplemented_trait_cannot_register() {
        // Types not implementing CalculationDomain cannot register (Req 6 Scen 4)
        // 编译期检查：register 只接受 Box<dyn CalculationDomain>
        // 以下代码若取消注释将无法编译：
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
        // 覆盖 is_arithmetic_only 中 Complex/Matrix/List/BigNumber → false 分支（line 177）
        assert!(!is_arithmetic_only(&AstNode::Complex(1.0, 2.0)));
        assert!(!is_arithmetic_only(&AstNode::Matrix(vec![vec![AstNode::Number(1.0)]])));
        assert!(!is_arithmetic_only(&AstNode::List(vec![AstNode::Number(1.0)])));
        assert!(!is_arithmetic_only(&AstNode::BigNumber("123".to_string())));
    }

    #[test]
    fn test_collect_function_names_unary_op_branch() {
        // 覆盖 collect_function_names_recursive 的 UnaryOp 分支（lines 130-132）
        // -foo(1) → 路由失败时递归收集函数名，应处理 UnaryOp 节点
        let router = default_router();
        let ast = AstNode::UnaryOp(
            crate::core::types::UnaryOp::Neg,
            Box::new(AstNode::FunctionCall("foo".to_string(), vec![AstNode::Number(1.0)])),
        );
        let err = router.route(&ast).err().expect("expected error");
        assert!(err.to_string().contains("foo"), "错误信息应包含 'foo': {}", err);
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
}
