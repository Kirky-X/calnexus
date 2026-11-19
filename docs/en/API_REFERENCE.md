# 📘 CalNexus API Reference

> This document describes CalNexus's two public API families: the **expression evaluation API** (`evaluate()`, the full pipeline through parsing and domain routing) and the **direct API** (the `CalNexus` facade, programmatic calls that skip parsing).
> The direct API complements, rather than replaces, the expression evaluation API.

## 📋 Table of Contents

- [🎯 Overview](#-overview)
    - [API Design Principles](#api-design-principles)
    - [📦 Feature Notes](#-feature-notes)
- [🧱 Core API](#-core-api)
    - [Expression Evaluation Entry](#expression-evaluation-entry)
    - [CalNexus Facade](#calnexus-facade)
    - [Variable Binding](#variable-binding)
- [🔌 Group APIs](#-group-apis)
    - [Scalar Operations — cn.scalar()](#scalar-operations--cnscalar)
    - [Linear Algebra — cn.linalg()](#linear-algebra--cnlinalg)
    - [Data Analysis — cn.stats()](#data-analysis--cnstats)
    - [Symbolic Math — cn.symbolic()](#symbolic-math--cnsymbolic)
    - [Applied Math — cn.applied()](#applied-math--cnapplied)
- [🧱 Type Wrappers](#-type-wrappers)
- [🚨 Error Types](#-error-types)
- [🚪 Feature-Gated APIs](#-feature-gated-apis)
- [💡 Usage Examples](#-usage-examples)
- [📚 Related Documents](#-related-documents)

---

## 🎯 Overview

### API Design Principles

- **Two entries, one kernel**: the expression API targets user input (parse → canonicalize → cache → domain routing); the direct API targets programmatic calls (skipping parsing/canonicalization overhead); both share the same math function layer (`math/`).
- **One-way dependencies**: `api/` → `math/` → `core/`, strictly one-way; the direct API does not depend on `domains/`.
- **Zero dependencies by default**: `default = []`; the core API has no optional dependencies and can serve as an embedded computation engine.
- **Gated optional capabilities**: optional computation domains and numerical decompositions are independent features; without them they are invisible.

### 📦 Feature Notes

| Feature         | Modules enabled                                            | Notes                                                            |
| --------------- | --------------------------------------------------- | --------------------------------------------------------------- |
| _(default, none)_ | `scalar` / `linalg` / `stats` / `symbolic`          | Core API, zero extra dependencies                                            |
| `numerical`     | `linalg.eig/eig_general/svd/lu/qr/solve/matrix_exp` | Numerical linear algebra decompositions (nalgebra f64)                                |
| `unit`          | `applied.convert()`                                 | 8-dimension physical unit conversion                                              |
| `time`          | `applied.date/datetime/now/today/...`               | 13 time methods (the expression domain has 14 time functions, jiff 0.2 + IANA tzdb) |
| `fx`            | `applied.fx/fx_rate`                                | FX conversion (frankfurter / ExchangeRate-API dual sources + 3-level cache)      |

> For server features (`cli`/`http`/`mcp`/`server`/`icu`/`ratelimit`/`docs`/`observability`) see the Feature Gate Strategy section of [🏗️ Architecture](ARCHITECTURE.md).

---

## 🧱 Core API

### Expression Evaluation Entry

```rust
pub fn evaluate(
    expr: &str,
    ctx: &EvalContext,
    precision: Option<usize>,
    cache: &CacheManager,
) -> Result<(EvalResult, String, bool, Option<usize>), CalcError>

pub fn evaluate_with_router(
    expr: &str,
    ctx: &EvalContext,
    precision: Option<usize>,
    cache: &CacheManager,
    router: &DomainRouter,
) -> Result<(EvalResult, String, bool, Option<usize>), CalcError>
```

- `evaluate` orchestrates the five-stage pipeline: input validation → parse + canonicalize + timeout check → cache key construction → dispatch by precision mode (precision mode bypasses the router into BigRational; regular mode routes through `DomainRouter`).
- The return is a 4-tuple: `(result, hit domain name, whether it was a cache hit, formatted precision echo)`. The 3rd element `false` means a real evaluation this time (including the nondeterministic bypass), `true` means a cache hit (including single-flight follower sharing); the 4th element `Option<usize>` is the N from `precision(N, expr)` in the expression (in precision mode, the precision argument passed in), used by the output layer for formatting.
- `evaluate_with_router` allows injecting a custom `DomainRouter` (downstream can extend custom `CalculationDomain`s); `evaluate` binds the process-wide default router.
- Caching goes through `try_get_with` single-flight: concurrent identical keys are really evaluated once; nondeterministic functions (`now` / `today` / `fx` / `fx_rate`) bypass cache reads and writes.

### AST comparison node

`AstNode::Compare(CompareOp, Box<AstNode>, Box<AstNode>)` is parsed from `<`, `<=`, `>` and `>=`. `CompareOp` is re-exported at the crate root. Comparison nodes are consumed only by the `solve_ineq` domain; other domains return a domain error. A bare `=` is valid only inside `solve`, `solve_system` and `solve_ineq` arguments, and is parsed as an internal `eq(lhs, rhs)` node.

### Solve, optimization and regression expression functions

The `solve`, `optimize`, `matrix` and `statistics` domains expose these expression functions:

| Function | Result | Notes |
| -------- | ------ | ----- |
| `solve(lhs=rhs, var)` | root vector or complex list | exact roots up to degree 4, numeric beyond |
| `solve(eq, var, [a,b])` | numeric root vector | bracketed interval with Newton refinement |
| `solve_system([eqs], [vars], [x0])` | JSON `{solution, residual}` | finite-difference Jacobian, damped Newton |
| `solve_ineq(ineq, var)` | JSON `{intervals, roots}` | supports `< <= > >=` comparisons |
| `minimize(f, var, [a,b])` | `[x*, f(x*)]` | Brent minimizer |
| `lp(c, A_ub, b_ub)` | JSON `{status, x, objective}` | two-phase simplex, Bland rule |
| `lstsq(A, b)` | solution vector | requires `numerical` feature |
| `ols(y, X)` | JSON `{coefficients, r_squared, std_errors}` | requires `numerical` feature |

### CalNexus Facade

`CalNexus` is the facade struct of the direct API, holding the variable context and providing 5 group accessors:

```rust
use calnexus::CalNexus;

let cn = CalNexus::new();     // create a default instance
// or
let cn = CalNexus::default(); // same as above

// scalar operations
let result = cn.scalar().add(2.0, 3.0).unwrap();
assert_eq!(result.as_scalar(), Some(5.0));

// trigonometric functions
let result = cn.scalar().sin(std::f64::consts::FRAC_PI_2).unwrap();
assert_eq!(result.as_scalar(), Some(1.0));
```

Group accessors and implementations:

| Accessor          | trait           | Methods | Coverage                                                                                                       |
| --------------- | --------------- | ------ | ---------------------------------------------------------------------------------------------------------- |
| `cn.scalar()`   | `ScalarMath`    | 53 + 1 | Arithmetic(8) + scientific functions(30) + precision(1) + number theory(9) + combinatorics(5); plus the inherent method `derivative`(1, requires the `numerical` feature) |
| `cn.linalg()`   | `LinearAlgebra` | 21     | Matrix(8) + vector(6) + numerical decompositions(7)                                                                            |
| `cn.stats()`    | `DataAnalysis`  | 32     | Basic statistics(8) + distributions(16) + hypothesis tests(3) + correlation(2) + regression(3)                                                   |
| `cn.symbolic()` | `SymbolicMath`  | 21     | Symbolic calculus(5) + polynomial(6) + complex(9) + equation solving(1)                                                            |
| `cn.applied()`  | `AppliedMath`   | 16     | Time(13) + unit(1) + fx(2), feature-gated                                                                 |

### Variable Binding

```rust
cn.set_var("x", 10.0);
assert_eq!(cn.get_var("x"), Some(10.0));

cn.clear_vars();
assert_eq!(cn.get_var("x"), None);
```

Variables are managed through `RwLock<EvalContext>`, supporting concurrent multi-threaded reads and exclusive writes.

---

## 🔌 Group APIs

### Scalar Operations — cn.scalar()

#### Arithmetic

| Method        | Signature                                               | Notes                 |
| ----------- | -------------------------------------------------- | -------------------- |
| `add`       | `(a: f64, b: f64) → Result<EvalResult, CalcError>` | Addition                 |
| `sub`       | `(a: f64, b: f64) → Result<EvalResult, CalcError>` | Subtraction                 |
| `mul`       | `(a: f64, b: f64) → Result<EvalResult, CalcError>` | Multiplication                 |
| `div`       | `(a: f64, b: f64) → Result<EvalResult, CalcError>` | Division (errors on division by zero) |
| `pow`       | `(a: f64, b: f64) → Result<EvalResult, CalcError>` | Power               |
| `rem`       | `(a: f64, b: f64) → Result<EvalResult, CalcError>` | Modulo                 |
| `factorial` | `(n: u64) → Result<EvalResult, CalcError>`         | Factorial                 |
| `abs`       | `(x: f64) → Result<EvalResult, CalcError>`         | Absolute value               |

#### Scientific Functions

| Method                                   | Signature                                                  | Notes                               |
| -------------------------------------- | ----------------------------------------------------- | ---------------------------------- |
| `sin`                                  | `(x: f64) → Result<EvalResult, CalcError>`            | Sine                               |
| `cos`                                  | `(x: f64) → Result<EvalResult, CalcError>`            | Cosine                               |
| `tan`                                  | `(x: f64) → Result<EvalResult, CalcError>`            | Tangent                               |
| `asin`                                 | `(x: f64) → Result<EvalResult, CalcError>`            | Arcsine                             |
| `acos`                                 | `(x: f64) → Result<EvalResult, CalcError>`            | Arccosine                               |
| `atan`                                 | `(x: f64) → Result<EvalResult, CalcError>`            | Arctangent                               |
| `ln`                                   | `(x: f64) → Result<EvalResult, CalcError>`            | Natural logarithm                           |
| `log`                                  | `(x: f64, base: f64) → Result<EvalResult, CalcError>` | Logarithm with arbitrary base                       |
| `exp`                                  | `(x: f64) → Result<EvalResult, CalcError>`            | Exponential function                           |
| `sinh`                                 | `(x: f64) → Result<EvalResult, CalcError>`            | Hyperbolic sine                           |
| `cosh`                                 | `(x: f64) → Result<EvalResult, CalcError>`            | Hyperbolic cosine                           |
| `tanh`                                 | `(x: f64) → Result<EvalResult, CalcError>`            | Hyperbolic tangent                           |
| `gamma`                                | `(x: f64) → Result<EvalResult, CalcError>`            | Gamma function                         |
| `erf`                                  | `(x: f64) → Result<EvalResult, CalcError>`            | Error function                           |
| `bessel_j0` / `bessel_j1`              | `(x: f64) → Result<EvalResult, CalcError>`            | Bessel J0/J1 of the first kind (libm fdlibm) |
| `bessel_j`                             | `(n: u32, x: f64) → Result<EvalResult, CalcError>`    | Integer-order Bessel of the first kind (0..=100)     |
| `bessel_y0` / `bessel_y1` / `bessel_y` | `(x: f64) / (n: u32, x: f64) → ...`                   | Bessel of the second kind (x > 0)             |
| `bessel_i0` / `bessel_i1` / `bessel_i` | `(x: f64) / (n: u32, x: f64) → ...`                   | Modified Bessel of the first kind (A&S 9.8)       |
| `bessel_k0` / `bessel_k1` / `bessel_k` | `(x: f64) / (n: u32, x: f64) → ...`                   | Modified Bessel of the second kind (x > 0)         |
| `beta`                                 | `(a: f64, b: f64) → Result<EvalResult, CalcError>`    | Beta function (a, b > 0)              |
| `zeta`                                 | `(s: f64) → Result<EvalResult, CalcError>`            | Riemann Zeta (s ≠ 1)              |
| `airy_ai` / `airy_bi`                  | `(x: f64) → Result<EvalResult, CalcError>`            | Airy functions                          |

#### Numerical Root Finding and Integration (expression level)

`root` and `integral` take expressions as input (not numeric arguments) and route in the `scientific` domain.

| Method       | Signature                                                                      | Notes                                                             |
| ---------- | ------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| `root`     | `(expr: &str, var: &str, a: f64, b: f64) → Result<EvalResult, CalcError>` | Brent root finding: finds a root of `var` in `expr`; the root must lie in the closed interval `[a, b]` |
| `integral` | `(expr: &str, var: &str, a: f64, b: f64) → Result<EvalResult, CalcError>` | Gauss-Kronrod 15 definite integral, falling back to adaptive Simpson when the error estimate exceeds the limit          |

Parameter semantics:

- `expr` — the expression string containing the variable `var` to evaluate; `var` must be a bare variable name.
- `var` — the variable name the root finding/integration targets (e.g. `x`).
- `a` / `b` — for root finding, the search interval endpoints (must bracket a root); for integration, the interval endpoints. When `a > b` the integral is negative.

Return shape: a scalar result (`EvalResult::Scalar`); JSON like `{"cache":"miss","domain":"scientific","result":<f64>,"v":1}`.

Error cases:

| Error                 | Trigger                                                  |
| -------------------- | --------------------------------------------------------- |
| `Eval` (argument count)   | `root` / `integral` called with an argument count other than 4                      |
| `Domain` (variable name)   | `var` is not a bare variable in the expression                                |
| `Domain` (unbracketed root) | No sign change of the expression within `root`'s interval `[a, b]`, so Brent cannot converge |
| `NaNOrInf`           | NaN/Inf occurs during evaluation                                    |
| `Domain` (integration interval) | Illegal integration interval endpoints (e.g. a or b non-finite)                      |

Example:

```bash
# Compute sqrt(2): root(x^2-2, x, 1, 2) → 1.414213562373137
calnexus --expression "root(x^2-2,x,1,2)"

# ∫₀^π sin(x) dx: integral(sin(x),x,0,pi) → 1.9999999999999942
calnexus --expression "integral(sin(x),x,0,pi)"
```

#### Numerical Differentiation

`derivative` also takes an expression as input, but is **exposed only via the API** (`cn.scalar().derivative(...)`),
not added to the expression routing whitelist — so it is not in the `--list-functions` catalog; requires the `numerical` feature.

| Method         | Signature                                                           | Notes                                                                                |
| ------------ | -------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `derivative` | `(f: &str, var: &str, x: f64) → Result<EvalResult, CalcError>` | Derivative of the expression `f` with respect to `var` at `x`: central differences + Richardson extrapolation (error ≈ 1e-10) |

Parameter semantics:

- `f` — the expression string to differentiate (e.g. `"sin(x)"`).
- `var` — the variable name to differentiate with respect to (e.g. `"x"`).
- `x` — the point to differentiate at.

When the expression evaluates to a non-scalar, a `Domain` error is returned (i18n key `msg.numerical.derivative_not_scalar`).

```rust
let cn = calnexus::CalNexus::new();
// the derivative of x^2 with respect to x at 3 is 6
let d = cn.scalar().derivative("x^2", "x", 3.0).unwrap();
assert_eq!(d.as_scalar(), Some(6.0));
```

#### Precision

| Method             | Signature                                                          | Notes                     |
| ---------------- | ------------------------------------------------------------- | ------------------------- |
| `precision_eval` | `(digits: usize, expr: &str) → Result<EvalResult, CalcError>` | BigRational arbitrary-precision evaluation |

#### Number Theory

| Method           | Signature                                                                                 | Notes             |
| -------------- | ------------------------------------------------------------------------------------ | ---------------- |
| `gcd`          | `(a: &BigNumber, b: &BigNumber) → Result<EvalResult, CalcError>`                     | Greatest common divisor       |
| `lcm`          | `(a: &BigNumber, b: &BigNumber) → Result<EvalResult, CalcError>`                     | Least common multiple       |
| `is_prime`     | `(n: &BigNumber) → Result<EvalResult, CalcError>`                                    | Primality test         |
| `prime_sieve`  | `(n: u64) → Result<EvalResult, CalcError>`                                           | Prime sieve           |
| `mod_pow`      | `(base: &BigNumber, exp: &BigNumber, m: &BigNumber) → Result<EvalResult, CalcError>` | Modular exponentiation         |
| `mod_inverse`  | `(a: &BigNumber, m: &BigNumber) → Result<EvalResult, CalcError>`                     | Modular multiplicative inverse           |
| `euler_phi`    | `(n: &BigNumber) → Result<EvalResult, CalcError>`                                    | Euler's totient         |
| `crt`          | `(remainders: &[BigNumber], moduli: &[BigNumber]) → Result<EvalResult, CalcError>`   | Chinese Remainder Theorem     |
| `discrete_log` | `(g: &BigNumber, h: &BigNumber, p: &BigNumber) → Result<EvalResult, CalcError>`      | Discrete logarithm (BSGS) |

#### Combinatorics

| Method              | Signature                                               | Notes               |
| ----------------- | -------------------------------------------------- | ---------------- |
| `perm`            | `(n: u64, k: u64) → Result<EvalResult, CalcError>` | Permutations P(n,k)      |
| `comb`            | `(n: u64, k: u64) → Result<EvalResult, CalcError>` | Combinations C(n,k)      |
| `catalan`         | `(n: u64) → Result<EvalResult, CalcError>`         | Catalan number         |
| `stirling_first`  | `(n: u64, k: u64) → Result<EvalResult, CalcError>` | Stirling number of the first kind |
| `stirling_second` | `(n: u64, k: u64) → Result<EvalResult, CalcError>` | Stirling number of the second kind |

### Linear Algebra — cn.linalg()

#### Matrix Operations

| Method         | Signature                                                       | Notes       |
| ------------ | ---------------------------------------------------------- | ------- |
| `det`        | `(m: &Matrix) → Result<EvalResult, CalcError>`             | Matrix determinant |
| `inverse`    | `(m: &Matrix) → Result<EvalResult, CalcError>`             | Matrix inversion   |
| `transpose`  | `(m: &Matrix) → Result<EvalResult, CalcError>`             | Matrix transpose   |
| `identity`   | `(n: usize) → Result<EvalResult, CalcError>`               | Identity matrix   |
| `mat_add`    | `(a: &Matrix, b: &Matrix) → Result<EvalResult, CalcError>` | Matrix addition   |
| `mat_sub`    | `(a: &Matrix, b: &Matrix) → Result<EvalResult, CalcError>` | Matrix subtraction   |
| `mat_mul`    | `(a: &Matrix, b: &Matrix) → Result<EvalResult, CalcError>` | Matrix multiplication   |
| `scalar_mul` | `(s: f64, m: &Matrix) → Result<EvalResult, CalcError>`     | Scalar × matrix |

#### Vector Operations

| Method         | Signature                                                       | Notes     |
| ------------ | ---------------------------------------------------------- | ------- |
| `dot`        | `(a: &Vector, b: &Vector) → Result<EvalResult, CalcError>` | Vector dot product |
| `cross`      | `(a: &Vector, b: &Vector) → Result<EvalResult, CalcError>` | Vector cross product |
| `normalize`  | `(a: &Vector) → Result<EvalResult, CalcError>`             | Normalize   |
| `magnitude`  | `(a: &Vector) → Result<EvalResult, CalcError>`             | Magnitude     |
| `vector_add` | `(a: &Vector, b: &Vector) → Result<EvalResult, CalcError>` | Vector addition |
| `vector_sub` | `(a: &Vector, b: &Vector) → Result<EvalResult, CalcError>` | Vector subtraction |

#### Numerical Decompositions (`numerical` feature)

| Method          | Signature                                                       | Notes                                                                                        |
| ------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `eig`         | `(m: &Matrix) → Result<EvalResult, CalcError>`             | Eigenvalues/eigenvectors of real symmetric matrices                                                                   |
| `eig_general` | `(m: &Matrix) → Result<EvalResult, CalcError>`             | Eigenvalues and eigenvectors of any real square matrix (`{"values_re","values_im","vectors_re","vectors_im"}`, including complex) |
| `svd`         | `(m: &Matrix) → Result<EvalResult, CalcError>`             | Singular value decomposition                                                                                  |
| `lu`          | `(m: &Matrix) → Result<EvalResult, CalcError>`             | LU decomposition                                                                                     |
| `qr`          | `(m: &Matrix) → Result<EvalResult, CalcError>`             | QR decomposition                                                                                     |
| `solve`       | `(a: &Matrix, b: &Vector) → Result<EvalResult, CalcError>` | Linear system solving                                                                              |
| `matrix_exp`  | `(m: &Matrix) → Result<EvalResult, CalcError>`             | Matrix exponential                                                                                    |

### Data Analysis — cn.stats()

#### Basic Statistics

| Method       | Signature                                             | Notes   |
| ---------- | ------------------------------------------------ | ------ |
| `mean`     | `(data: &[f64]) → Result<EvalResult, CalcError>` | Mean   |
| `variance` | `(data: &[f64]) → Result<EvalResult, CalcError>` | Variance   |
| `std`      | `(data: &[f64]) → Result<EvalResult, CalcError>` | Standard deviation |
| `median`   | `(data: &[f64]) → Result<EvalResult, CalcError>` | Median |
| `min`      | `(data: &[f64]) → Result<EvalResult, CalcError>` | Minimum |
| `max`      | `(data: &[f64]) → Result<EvalResult, CalcError>` | Maximum |
| `sum`      | `(data: &[f64]) → Result<EvalResult, CalcError>` | Sum   |
| `count`    | `(data: &[f64]) → Result<EvalResult, CalcError>` | Count   |

#### Distribution Functions

| Method          | Signature                                                            | Notes             |
| ------------- | --------------------------------------------------------------- | ------------ |
| `norm_pdf`    | `(x: f64, mu: f64, sigma: f64) → Result<EvalResult, CalcError>` | Normal PDF |
| `norm_cdf`    | `(x: f64, mu: f64, sigma: f64) → Result<EvalResult, CalcError>` | Normal CDF |
| `norm_inv`    | `(p: f64, mu: f64, sigma: f64) → Result<EvalResult, CalcError>` | Normal inverse function   |
| `t_pdf`       | `(x: f64, df: f64) → Result<EvalResult, CalcError>`             | t distribution PDF   |
| `t_cdf`       | `(x: f64, df: f64) → Result<EvalResult, CalcError>`             | t distribution CDF   |
| `t_inv`       | `(p: f64, df: f64) → Result<EvalResult, CalcError>`             | t distribution inverse function   |
| `chi2_pdf`    | `(x: f64, k: f64) → Result<EvalResult, CalcError>`              | Chi-square PDF |
| `chi2_cdf`    | `(x: f64, k: f64) → Result<EvalResult, CalcError>`              | Chi-square CDF |
| `chi2_inv`    | `(p: f64, k: f64) → Result<EvalResult, CalcError>`              | Chi-square inverse function   |
| `f_pdf`       | `(x: f64, d1: f64, d2: f64) → Result<EvalResult, CalcError>`    | F distribution PDF |
| `f_cdf`       | `(x: f64, d1: f64, d2: f64) → Result<EvalResult, CalcError>`    | F distribution CDF |
| `f_inv`       | `(p: f64, d1: f64, d2: f64) → Result<EvalResult, CalcError>`    | F distribution inverse function   |
| `poisson_pmf` | `(k: f64, lambda: f64) → Result<EvalResult, CalcError>`         | Poisson PMF |
| `poisson_cdf` | `(k: f64, lambda: f64) → Result<EvalResult, CalcError>`         | Poisson CDF |
| `binom_pmf`   | `(k: f64, n: f64, p: f64) → Result<EvalResult, CalcError>`      | Binomial PMF |
| `binom_cdf`   | `(k: f64, n: f64, p: f64) → Result<EvalResult, CalcError>`      | Binomial CDF |

#### Hypothesis Tests

| Method         | Signature                                                                   | Notes          |
| ------------ | ---------------------------------------------------------------------- | ---------- |
| `t_test_one` | `(data: &[f64], mu: f64) → Result<EvalResult, CalcError>`              | One-sample t test |
| `t_test_two` | `(a: &[f64], b: &[f64]) → Result<EvalResult, CalcError>`               | Two-sample t test |
| `chi2_test`  | `(observed: &[f64], expected: &[f64]) → Result<EvalResult, CalcError>` | Chi-square test      |

#### Correlation Analysis

| Method       | Signature                                                     | Notes             |
| ---------- | -------------------------------------------------------- | ---------- |
| `pearson`  | `(x: &[f64], y: &[f64]) → Result<EvalResult, CalcError>` | Pearson correlation coefficient |
| `spearman` | `(x: &[f64], y: &[f64]) → Result<EvalResult, CalcError>` | Spearman rank correlation  |

#### Regression Analysis

| Method        | Signature                                                                    | Notes       |
| ----------- | ----------------------------------------------------------------------- | ------- |
| `lin_reg`   | `(x: &[f64], y: &[f64]) → Result<EvalResult, CalcError>`                | Linear regression   |
| `poly_reg`  | `(x: &[f64], y: &[f64], degree: usize) → Result<EvalResult, CalcError>` | Polynomial regression |
| `multi_reg` | `(x: &[Vec<f64>], y: &[f64]) → Result<EvalResult, CalcError>`           | Multivariate regression   |

### Symbolic Math — cn.symbolic()

#### Symbolic Calculus

| Method            | Signature                                                                                 | Notes       |
| --------------- | ------------------------------------------------------------------------------------ | ---------- |
| `differentiate` | `(expr: &str, var: &str) → Result<EvalResult, CalcError>`                            | Symbolic differentiation   |
| `integrate`     | `(expr: &str, var: &str) → Result<EvalResult, CalcError>`                            | Symbolic integration   |
| `simplify`      | `(expr: &str) → Result<EvalResult, CalcError>`                                       | Expression simplification |
| `limit`         | `(expr: &str, var: &str, target: f64) → Result<EvalResult, CalcError>`               | Limits       |
| `taylor_expand` | `(expr: &str, var: &str, center: f64, order: usize) → Result<EvalResult, CalcError>` | Taylor expansion   |

#### Polynomial Operations

| Method         | Signature                                                               | Notes       |
| ------------ | ------------------------------------------------------------------ | ---------- |
| `poly_add`   | `(a: &Polynomial, b: &Polynomial) → Result<EvalResult, CalcError>` | Polynomial addition |
| `poly_sub`   | `(a: &Polynomial, b: &Polynomial) → Result<EvalResult, CalcError>` | Polynomial subtraction |
| `poly_mul`   | `(a: &Polynomial, b: &Polynomial) → Result<EvalResult, CalcError>` | Polynomial multiplication |
| `poly_div`   | `(a: &Polynomial, b: &Polynomial) → Result<EvalResult, CalcError>` | Polynomial division |
| `poly_roots` | `(p: &Polynomial) → Result<EvalResult, CalcError>`                 | Polynomial roots |
| `poly_eval`  | `(p: &Polynomial, x: f64) → Result<EvalResult, CalcError>`         | Polynomial evaluation |

#### Complex Operations

| Method           | Signature                                                         | Notes     |
| -------------- | ------------------------------------------------------------ | ------ |
| `complex_add`  | `(a: &Complex, b: &Complex) → Result<EvalResult, CalcError>` | Complex addition |
| `complex_sub`  | `(a: &Complex, b: &Complex) → Result<EvalResult, CalcError>` | Complex subtraction |
| `complex_mul`  | `(a: &Complex, b: &Complex) → Result<EvalResult, CalcError>` | Complex multiplication |
| `complex_div`  | `(a: &Complex, b: &Complex) → Result<EvalResult, CalcError>` | Complex division |
| `complex_abs`  | `(z: &Complex) → Result<EvalResult, CalcError>`              | Complex modulus   |
| `complex_arg`  | `(z: &Complex) → Result<EvalResult, CalcError>`              | Complex argument |
| `complex_conj` | `(z: &Complex) → Result<EvalResult, CalcError>`              | Complex conjugate   |
| `complex_exp`  | `(z: &Complex) → Result<EvalResult, CalcError>`              | Complex exponential   |
| `complex_ln`   | `(z: &Complex) → Result<EvalResult, CalcError>`              | Complex logarithm   |

#### Equation Solving

| Method             | Signature                                                                                             | Notes         |
| ---------------- | ------------------------------------------------------------------------------------------------ | ---------- |
| `solve_equation` | `(expr: &str, var: &str, method: &str, options: Option<&[f64]>) → Result<EvalResult, CalcError>` | Numerical equation solving |

`solve_equation` supports three methods:

- `"newton"`: Newton's method, options = `Some(&[x0])` (initial guess)
- `"bisection"`: bisection, options = `Some(&[a, b])` (interval)
- `"brent"`: Brent's method, options = `Some(&[a, b])` (interval)

### Applied Math — cn.applied()

#### Time (`time` feature)

| Method             | Signature                                                                          | Notes                           |
| ---------------- | ----------------------------------------------------------------------------- | ------------------------------ |
| `date`           | `(date_str: &str) → Result<EvalResult, CalcError>`                            | Date construction                       |
| `datetime`       | `(datetime_str: &str, tz: Option<&str>) → Result<EvalResult, CalcError>`      | Datetime construction                   |
| `timestamp`      | `(datetime_str: &str) → Result<EvalResult, CalcError>`                        | Date to timestamp                   |
| `from_timestamp` | `(secs: i64, tz: Option<&str>) → Result<EvalResult, CalcError>`               | Timestamp to date                   |
| `now`            | `(tz: Option<&str>) → Result<EvalResult, CalcError>`                          | Current time (nondeterministic, bypasses the cache) |
| `today`          | `(tz: Option<&str>) → Result<EvalResult, CalcError>`                          | Current date (nondeterministic, bypasses the cache) |
| `date_add`       | `(date: &str, n: i64, unit: &str) → Result<EvalResult, CalcError>`            | Date addition                       |
| `date_diff`      | `(a: &str, b: &str, unit: Option<&str>) → Result<EvalResult, CalcError>`      | Date difference                         |
| `format_date`    | `(date: &str, fmt: &str, tz: Option<&str>) → Result<EvalResult, CalcError>`   | Date formatting                     |
| `reformat_date`  | `(input: &str, from_fmt: &str, to_fmt: &str) → Result<EvalResult, CalcError>` | Date format conversion                   |
| `weekday`        | `(date: &str) → Result<EvalResult, CalcError>`                                | Day of week                         |
| `day_of_year`    | `(date: &str) → Result<EvalResult, CalcError>`                                | Day of year                     |
| `is_leap_year`   | `(year: i64) → Result<EvalResult, CalcError>`                                 | Leap year or not                       |

#### Unit Conversion (`unit` feature)

| Method      | Signature                                                                 | Notes     |
| --------- | -------------------------------------------------------------------- | ------ |
| `convert` | `(value: f64, from: &str, to: &str) → Result<EvalResult, CalcError>` | Unit conversion |

#### FX Conversion (`fx` feature)

| Method      | Signature                                                                  | Notes                           |
| --------- | --------------------------------------------------------------------- | ------------------------------ |
| `fx`      | `(amount: f64, from: &str, to: &str) → Result<EvalResult, CalcError>` | FX conversion (nondeterministic, bypasses the cache) |
| `fx_rate` | `(from: &str, to: &str) → Result<EvalResult, CalcError>`              | Rate query (nondeterministic, bypasses the cache) |

---

## 🧱 Type Wrappers

The direct API uses 5 type-safe wrappers with two-way conversion to/from `EvalResult`:

| Wrapper       | Wrapped type          | Constructor                                            |
| ------------ | ----------------- | --------------------------------------------------- |
| `Matrix`     | `Vec<Vec<f64>>`   | `Matrix::from_rows(&[&[f64]])`                      |
| `Vector`     | `Vec<f64>`        | `Vector::new(&[f64])`                               |
| `Complex`    | `Complex64`       | `Complex::new(re, im)`                              |
| `Polynomial` | `Vec<f64>` (ascending powers) | `Polynomial::new(&[f64])`                           |
| `BigNumber`  | `BigInt`          | `BigNumber::from_i64(v)` / `BigNumber::new(bigint)` |

### Conversion

```rust
use calnexus::{Matrix, EvalResult};

// wrapper → EvalResult
let m = Matrix::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]);
let result: EvalResult = m.into();

// EvalResult → wrapper
let m2 = Matrix::try_from(result).unwrap();
```

---

## 🚨 Error Types

The error type of every API is `CalcError`, whose `kind` field is the `ErrorKind` enum:

| ErrorKind               | Exit code | Server mapping (HTTP `code`)                | Trigger                                 |
| ----------------------- | ------ | ---------------------------------------- | ---------------------------------------- |
| `Parse`                 | 1      | 400 BAD_REQUEST                          | Expression syntax error                           |
| `Eval`                  | 1      | 400 BAD_REQUEST                          | Evaluation failure (domain, unimplemented operation, etc.)         |
| `Overflow`              | 1      | 400 BAD_REQUEST                          | Numeric overflow                                 |
| `DivisionByZero`        | 1      | 400 BAD_REQUEST                          | Division by zero                                     |
| `Domain`                | 1      | 400 BAD_REQUEST                          | Function domain error                           |
| `Depth`                 | 1      | 400 BAD_REQUEST                          | AST depth limit exceeded (`MAX_AST_DEPTH=256`) |
| `NaNOrInf`              | 1      | 400 BAD_REQUEST                          | NaN / infinite result                           |
| `UndefinedSymbol`       | 1      | 400 BAD_REQUEST                          | Unbound variable (with `--var` / `:let` hint)   |
| `Usage`                 | 2      | 400 BAD_REQUEST (message prefixed `Usage:`) | Usage/argument error                            |
| `Timeout`               | 3      | 503 UNAVAILABLE                          | `--timeout` exceeded                         |
| `DependencyUnavailable` | 3      | 503 UNAVAILABLE                          | Upstream dependency failure (fx rate source unreachable)          |

> **422 does not come from `ErrorKind`**: HTTP 422 `UNPROCESSABLE_ENTITY` (MCP `ValidationError`) is produced before evaluation by a separate request-body validation layer (`server/types.rs::validate()`: expr ≤ 4096 chars, vars ≤ 1024 keys, precision ≤ 10000); the CLI form has no such layer — the corresponding constraints are enforced by clap / `--precision` validation (exit code 2). The HTTP error body is the sdforge UnifiedError (`code`/`message`/`trace_id`/`field`), while MCP keeps `ApiError` serialization (the `type` protocol name).

`CalcError` carries `message` (localized text, negotiated via `--lang` / HTTP `lang`), `hint` (fix suggestion), and `source_detail` (error chain). The exit-code contract is given uniformly by `ErrorKind::exit_code()`: 0 = success, 1 = computation error, 2 = usage error, 3 = timeout/upstream unavailable.

---

## 🚪 Feature-Gated APIs

| Feature        | Public items                                                                   | Notes                                                              |
| ----------- | ------------------------------------------------------------------------ | ----------------------------------------------------------------- |
| `time`      | `TimeDomain`, `applied()` time methods                                       | jiff 0.2 + embedded IANA tzdb                                         |
| `unit`      | `UnitDomain`, `applied().convert()`                                      | 8 dimensions + affine temperature                                                 |
| `fx`        | `FxDomain`, `applied().fx/fx_rate`                                       | frankfurter / er-api dual sources (`CALNEXUS_FX_PROVIDER`) + 3-level cache + breaker; `CALNEXUS_FX_*` environment variables |
| `numerical` | `linalg()` numerical decomposition methods                                                  | nalgebra f64 approximations                                                 |
| `server`    | `HttpServer` / `build_router` / `McpServer` / `build_mcp_server`         | HTTP + MCP services                                                   |
| `icu`       | Affects the BCP-47 parsing implementation behind `I18n` (`I18n` / `Lang` types always available, not gated) | Enables ICU4X locale-tag parsing; without it, `from_str` degrades to simple string matching |
| `wasm-api`  | `wasm::evaluate` / `wasm::list_functions`                                | wasm32 targets only; enabling on non-wasm is a compile-time error                      |
| `egraph`    | `symbolic::simplify` adds E-graph saturation                                   | Depends on `egg 0.9`; off by default, greedy fallback on failure                            |

### wasm API (`wasm-api`, wasm32 only)

```rust
// Rust-side signatures (exported to JS via wasm-bindgen)
pub fn evaluate(expr: &str) -> Result<String, JsValue>;
pub fn list_functions() -> String;
```

- `evaluate("2+3")` → `{"v":1,"result":{"Scalar":5.0}}`
- `evaluate("")` → `Err`, payload `{"v":1,"error":{"code":2,"kind":"usage","message":"expression must not be empty"}}`
- Computation errors carry `code` 1; dependency unavailable/timeout is 3; `kind` is the snake_case name of `ErrorKind`
- `list_functions()` → `[{"domain":"scientific","functions":[...]}, ...]`

Build (CI gate): `cargo build --lib --target wasm32-unknown-unknown --features cli,wasm-api`.

All domain types (`ArithmeticDomain` and the other 14) implement the `CalculationDomain` trait and can be injected via `evaluate_with_router`; `build_default_router()` registers all enabled domains.

---

## 💡 Usage Examples

### Scenario Selection

| Scenario               | Recommended API     | Reason                |
| ------------------ | ------------ | ------------------- |
| User-input expression     | `evaluate()` | Needs parsing + domain routing   |
| CLI/REPL/Server    | `evaluate()` | Expression string entry    |
| Programmatic calls of known operations | Direct API     | Skips parsing/canonicalization overhead |
| Embedded computation engine     | Direct API     | No expression-parsing dependency  |

### Basic Calls

```rust
use calnexus::{CalNexus, BigNumber, Matrix, Vector};

let cn = CalNexus::new();

// matrix determinant
let m = Matrix::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]);
let r = cn.linalg().det(&m).unwrap();
assert_eq!(r.as_scalar(), Some(-2.0));

// vector dot product
let a = Vector::new(&[1.0, 2.0, 3.0]);
let b = Vector::new(&[4.0, 5.0, 6.0]);
let r = cn.linalg().dot(&a, &b).unwrap();
assert_eq!(r.as_scalar(), Some(32.0));

// Chinese Remainder Theorem: x≡2(mod 3), x≡3(mod 5), x≡2(mod 7) → x=23
let r = vec![BigNumber::from_i64(2), BigNumber::from_i64(3), BigNumber::from_i64(2)];
let m = vec![BigNumber::from_i64(3), BigNumber::from_i64(5), BigNumber::from_i64(7)];
let result = cn.scalar().crt(&r, &m).unwrap();
// result = BigInt(23)

// linear regression
let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
let r = cn.stats().lin_reg(&x, &y).unwrap();
// returns JSON: {"slope": 2.0, "intercept": 0.0, "r_squared": 1.0}

// equation solving: x²-2=0, Newton's method
let r = cn.symbolic().solve_equation("x^2 - 2", "x", "newton", Some(&[1.5])).unwrap();
let root = r.as_scalar().unwrap();
assert!((root - std::f64::consts::SQRT_2).abs() < 1e-10);

// unit conversion: 1000 meters → 1 kilometer
let r = cn.applied().convert(1000.0, "m", "km").unwrap();
assert_eq!(r.as_scalar(), Some(1.0));
```

### Custom Router Injection

```rust
use calnexus::{evaluate_with_router, CacheManager, DomainRouter, EvalContext};

let router = DomainRouter::new(); // empty router (no built-in domains)
let cache = CacheManager::new();
let ctx = EvalContext::new();
// with an empty router every expression returns a routing error, proving the injected router is consumed
let err = evaluate_with_router("1+1", &ctx, None, &cache, &router).unwrap_err();
```

---

## 📚 Related Documents

- [📖 User Guide](USER_GUIDE.md) — CLI usage tutorial
- [🏗️ Architecture](ARCHITECTURE.md) — Module layout, dependency direction, and data flow
- [📈 Performance Guide](PERFORMANCE.md) — Direct API vs expression path performance comparison
- [📦 Online API docs](https://docs.rs/calnexus) — rustdoc generated by docs.rs
