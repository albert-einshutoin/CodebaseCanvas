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
- CI integration
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

Do not add a TypeScript analyzer fallback. Validate Oxc against the fixture before building recognizers on top of it.

The PoC has no backend. Do not add Next.js, React Flow, tRPC, Effect, a Worker API, or Cloudflare storage products unless the product scope is explicitly changed.

## Important architectural rule

The analyzer must produce a normalized graph model that is independent from the UI.

Do not expose raw AST nodes directly to the frontend.

Expected shape:

```ts
interface SystemGraph {
  nodes: GraphNode[];
  edges: GraphEdge[];
}
```

## First implementation sequence

1. Define graph schema.
2. Implement file discovery.
3. Validate Oxc parsing and resolution against the fixture.
4. Detect NestJS classes and decorators.
5. Detect constructor dependency injection.
6. Detect modules and controller routes.
7. Generate `graph.json`.
8. Build the React/Vite/Cytoscape.js canvas UI.
9. Add local graph selection and Zod validation.
10. Add selection and details panel.
11. Add search and focus mode.
12. Add Copy Context.
13. Deploy the static SPA to Cloudflare Workers Static Assets.

## Testing

At minimum:

- Rust unit tests for analyzer recognizers.
- Fixture NestJS project.
- Snapshot or structural graph tests.
- One end-to-end test proving the fixture can be analyzed and rendered.
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
