# Prompt for an AI Coding Agent

You are implementing a proof of concept called **CodebaseCanvas**.

Read all Markdown documents in this repository before implementing anything.

## Product intent

AI coding tools can generate code faster than humans can understand it.

CodebaseCanvas converts a codebase into a visual system model so humans can understand and maintain software without reading every source file.

This PoC is not an AI coding assistant and must not include a built-in LLM.

## Primary objective

Build the smallest working application that can:

1. Analyze a local TypeScript/NestJS repository.
2. Detect important NestJS and TypeScript entities.
3. Convert them into a normalized graph.
4. Display the graph as an interactive web canvas.
5. Allow users to inspect dependencies.
6. Export selected component context as Markdown for use in an external LLM.

## Engineering priorities

In priority order:

1. Correct analysis
2. Useful visualization
3. Simple architecture
4. Fast iteration
5. Extensibility

Do not optimize prematurely.

## Constraints

Do not implement:

- Authentication
- Billing
- Team functionality
- Built-in AI/LLM calls
- GitHub App
- CI failure ingestion as a product feature (repository development CI in Issue #31 is required)
- Runtime tracing
- Multi-language support

## Suggested implementation

Use:

- Rust for the repository analyzer and CLI
- Oxc for TypeScript parsing, semantic analysis, and module resolution
- Serde and `serde_json` for the graph output
- TypeScript, React, and Vite for the web UI
- Cytoscape.js for graph rendering
- Zod for validating imported graph data
- Vitest for web unit tests and Playwright for the browser test
- Cloudflare Workers Static Assets, the Cloudflare Vite plugin, and Wrangler for hosting

A single repository is enough.

Do not add a TypeScript analyzer fallback. Validate Oxc against the fixture and a representative real NestJS repository pinned to a commit before building recognizers on top of it (Issue #7).

The PoC has no backend. Do not add Next.js, React Flow, tRPC, Effect, a Worker API, or Cloudflare storage products unless the product scope is explicitly changed.

## Important architectural rule

The analyzer must produce a normalized graph model that is independent from the UI.

Do not expose raw AST nodes directly to the frontend.

Use [DATA_MODEL.md](DATA_MODEL.md) as the canonical v0.1 contract, including schemaVersion, metadata/call coverage, nodes, edges, and diagnostics. Do not maintain another reduced schema here.

`injects` means a requested runtime class token, never a resolved provider implementation. Do not infer injected-receiver method calls. Record skipped calls with scoped diagnostics and counts. Preserve multi-module membership independently from display parents, and use only the contract's bounded paths for Copy Context.

## First implementation sequence

Follow Epic #1 and the selected Issue for the authoritative dependency order. #3 owns the [wire validators and canonical IDs](DATA_MODEL.md#wire-validation-and-ownership), #4 the independent source fixture, #16 graph assembly and #19 file import. Reuse these APIs. All file access must follow the contract's repository I/O boundary. Preserve `analyzedAt` and require re-analysis/re-selection for changed source.

## Testing

At minimum:

- Rust unit tests for analyzer recognizers.
- Fixture NestJS project.
- Snapshot or structural graph tests.
- One end-to-end test that builds the current Rust checkout, freshly analyzes the fixture, and imports that exact graph through the production File API, Zod validation, and Canvas. Fail on analyzer errors; never substitute a saved graph.
- Negative cases for provider overrides/type-only DI, shared module membership, unknown call counts, and bounded context traversal/truncation.
- A recorded analysis-time and peak-memory measurement on a representative real repository before claiming a performance improvement.

## Avoid

- Complex microservice architecture
- Generic language framework too early
- Neo4j
- Kubernetes
- Custom DSL
- LLM-based static analysis
- Excessive abstractions

If a decision is ambiguous, choose the implementation that produces a working demonstrable PoC with the least complexity.

The final PoC should be easy to run locally and easy for another engineer or coding agent to understand.
