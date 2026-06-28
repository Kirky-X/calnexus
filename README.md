# CalNexus

A command-line math expression evaluator with 11 computation domains, symbolic calculus, REPL, and batch processing.

## Features

### 11 Computation Domains

| Domain | Priority | Functions |
|--------|----------|-----------|
| **Arithmetic** | 10 | `+`, `-`, `*`, `/`, `^`, `factorial`, `mod`, `abs` |
| **Scientific** | 20 | `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `ln`, `log`, `exp`, `sinh`, `cosh`, `tanh`, `gamma`, `erf` |
| **Statistics** | 20 | `mean`, `median`, `variance`, `stddev`, `sum`, `min`, `max` |
| **Precision** | 25 | `precision(N, expr)` — BigRational arbitrary precision |
| **NumberTheory** | 25 | `gcd`, `lcm`, `is_prime`, `prime_sieve`, `mod_inverse`, `mod_pow`, `euler_phi` |
| **Combinatorics** | 25 | `P`, `C`, `catalan`, `stirling` |
| **Polynomial** | 25 | `poly_add`, `poly_sub`, `poly_mul`, `poly_div`, `poly_eval`, `poly_diff`, `poly_integrate`, `roots`, `factor` |
| **Complex** | 30 | `complex(a,b)`, `re`, `im`, `conj`, `magnitude`, `phase` |
| **Matrix** | 30 | `det`, `transpose`, `inverse`, `trace` |
| **Vector** | 30 | `dot`, `cross`, `norm`, `angle`, `normalize`, `scalar_triple` |
| **Symbolic** | 30 | `diff`, `integrate`, `simplify`, `limit`, `taylor` |

### Three Modes

1. **Single expression** — `calnexus '2+3*4'`
2. **REPL** — `calnexus --repl` (interactive, with Tab completion and variable binding)
3. **Batch** — `calnexus --batch exprs.txt` (parallel evaluation with rayon)

## Installation

```bash
# Build from source
cargo install --path . --features cli

# Or clone and build
git clone https://github.com/kirky-x/calnexus.git
cd calnexus
cargo build --release --features cli
```

## Usage

### Single Expression

```bash
$ calnexus '2+3*4'
14

$ calnexus 'sin(pi/2)'
1

$ calnexus 'gcd(12, 18)'
6

$ calnexus 'factorial(5)'
120

$ calnexus --var x=3 'x^2 + 2*x + 1'
16
```

### Arbitrary Precision

```bash
$ calnexus --precision 50 '1/3'
0.33333333333333333333333333333333333333333333333333
```

### JSON Output

```bash
$ calnexus --json '2+3'
{"result":5,"domain":"arithmetic","cache":"miss"}
```

### Symbolic Calculus

```bash
$ calnexus 'diff(x^2, x)'
2*x

$ calnexus 'simplify(x+0)'
x

$ calnexus 'limit(sin(x)/x, x, 0)'
1

$ calnexus 'taylor(exp(x), x, 3)'
1+x+0.5*x^2+0.16666666666666666*x^3
```

### REPL Mode

```bash
$ calnexus --repl
CalNexus REPL — type :help for commands, :quit to exit
calnexus> :let x = 10
calnexus> x*2
= 20  [arithmetic]
calnexus> diff(x^2, x)
= 2*x  [symbolic]
calnexus> :vars
x = 10
calnexus> :quit
bye
```

### Batch Processing

```bash
$ cat exprs.txt
2+3
sin(0)
# This is a comment
diff(x^2, x)

$ calnexus --batch exprs.txt
line 1: 2+3 = 5  [arithmetic]
line 2: sin(0) = 0  [scientific]
line 4: diff(x^2, x) = 2*x  [symbolic]
summary: 3 total, 3 ok, 0 errors, 0 cache hits, 1.2ms
```

### Implicit Multiplication

```bash
$ calnexus --var x=3 '2x'
6

$ calnexus --var x=3 '3(x+1)'
12
```

## Architecture

```
parse() → AstCanonicalizer → CacheManager → DomainRouter → Domain::evaluate()
  │            │                    │              │                │
  │            │                    │              │                ├─ ArithmeticDomain
  │            │                    │              │                ├─ ScientificDomain
  │            │                    │              │                ├─ PrecisionDomain
  │            │                    │              │                ├─ NumberTheoryDomain
  │            │                    │              │                ├─ CombinatoricsDomain
  │            │                    │              │                ├─ PolynomialDomain
  │            │                    │              │                ├─ ComplexDomain
  │            │                    │              │                ├─ MatrixDomain
  │            │                    │              │                ├─ VectorDomain
  │            │                    │              │                └─ SymbolicDomain
```

- **Parser**: mathexpr-based, with implicit multiplication and complex number preprocessing
- **Canonicalizer**: constant folding, commutative sorting, S-expression canonical form
- **Cache**: Moka L1 cache (10000 entries, BLAKE3 key hash, thread-safe)
- **Router**: Priority-sorted domain dispatch (first `supports()` wins)

## Testing

```bash
# Run all tests
cargo test --features cli

# Release build (zero warnings)
cargo build --release --features cli
```

1343 tests (1134 lib + 79 CLI + 130 integration), release build with zero warnings.

## License

MIT
