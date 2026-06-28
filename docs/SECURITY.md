# Security Policy

## Supported Versions

CalNexus is currently supported with security updates for the following versions:

| Version | Supported          |
|---------|--------------------|
| 1.x     | ✅ Supported        |
| 0.x     | ❌ Not supported    |

Only the latest minor release within the `1.x` line receives security fixes. Please upgrade to the most recent release before reporting a vulnerability whenever possible.

## Reporting a Vulnerability

**Do NOT open a public GitHub issue** to report a security vulnerability.

If you believe you have discovered a security issue in CalNexus, please report it privately by emailing **[security@calnexus.dev](mailto:security@calnexus.dev)**. Your report should ideally include:

- A clear description of the vulnerability and its potential impact.
- Step-by-step instructions to reproduce the issue, including the exact expression or input that triggers it.
- The CalNexus version and platform you tested on.
- Any suggested remediation, if you have one.

Please do not disclose the issue publicly until a fix has been released and you have been given the all-clear.

## Response Timeline

- **Within 48 hours**: we will acknowledge receipt of your report.
- **Within 7 days**: we will provide an initial assessment, including whether the issue is confirmed and its severity.
- **As soon as feasible** (severity-dependent): we will prepare and release a fix, keeping you informed of progress.
- **After the fix is released**: we will coordinate a public disclosure with you and credit your contribution (unless you prefer to remain anonymous).

We follow a responsible disclosure model and ask that reporters do the same.

## Attack Surface

CalNexus is a **command-line math expression evaluator** with **no network functionality**, no file I/O on untrusted paths, and no external plugin loading. As a result, the practical attack surface is minimal. The areas we actively consider during security review include:

- **Expression injection / untrusted input**: malformed or adversarial expressions that could cause panics, infinite loops, or unintended resource consumption when evaluated.
- **Depth overflow / recursion limits**: deeply nested expressions that could overflow the parser/evaluator stack or exhaust memory. CalNexus enforces recursion and nesting limits; any bypass of these limits is a security-relevant bug.
- **Integer and floating-point edge cases**: inputs crafted to trigger overflow, NaN propagation, or other numeric misbehavior with security implications.

If your finding falls outside these categories but you still believe it has security impact, please report it anyway — we would rather review a false positive than miss a real issue.
