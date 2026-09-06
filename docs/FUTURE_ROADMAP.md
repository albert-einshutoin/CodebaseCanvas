# Future Roadmap

This file is intentionally non-binding for the PoC.

## v0.1 — System Canvas
- Repository analysis
- Architecture visualization
- Dependency inspection
- Context export

## v0.2 — Change Canvas
- Git base/head comparison
- Symbol-level diff
- Architecture change visualization
- Blast radius

## v0.3 — Failure Canvas
- GitHub Actions
- Test failures
- Stack traces
- Failure-path visualization
- Context export for user's LLM

## v0.4 — Health Canvas
- Duplicate logic
- Near-duplicate logic
- Circular dependencies
- High coupling
- Large classes
- Excessive fan-in / fan-out
- Change coupling

## v0.5 — Agent Integration
- MCP server
- Query graph from coding agents
- Retrieve scoped context
- No bundled LLM required

## v1+
- Runtime tracing
- OpenTelemetry
- Sentry integration
- Cross-repository analysis
- Non-engineer product lens
- Visual architecture editing
- Expected architecture vs actual implementation

The v0.1 boundary is the [versioned snapshot contract](DATA_MODEL.md). Later change views must not assume its IDs survive moves/refactors or introduce silent version migration. Health analysis is optional after the required PoC, as tracked separately by #29.
