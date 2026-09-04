# Technical Architecture — PoC

## High-level design

```text
Local Repository
      |
      v
Native Rust/Oxc Analyzer CLI
      |
      v
Local graph.json
      |
      v
React/Vite/Cytoscape.js Canvas
      ^
      |
Cloudflare Workers Static Assets
```

The browser loads `graph.json` from a user-selected local file. Cloudflare serves only the application assets; repository contents and graph data are not uploaded in the PoC.

## Selected stack

| Layer | Selection |
|---|---|
| Analyzer and CLI | Rust |
| TypeScript parser | Oxc parser, semantic analyzer, and resolver |
| Serialization | Serde and `serde_json` |
| Graph contract | Normalized JSON validated at the UI boundary |
| Web application | React, TypeScript, and Vite |
| Graph canvas | Cytoscape.js |
| UI validation | Zod |
| Unit tests | Rust built-in test harness and Vitest |
| Browser test | Playwright |
| Hosting | Cloudflare Workers Static Assets |
| Deployment tooling | Cloudflare Vite plugin and Wrangler |

Use Cargo for Rust dependencies and pnpm for the web application.

### Web application

Use a client-side React SPA built with Vite. Server-side rendering, React Server Components, and server actions are not required for an interactive code graph, so Next.js is not part of the PoC.

Use Cytoscape.js because the product is a graph viewer and analysis tool rather than a node editor. Render only the relevant level of detail and reveal methods or unrelated nodes on demand.

### Backend and infrastructure

The PoC has no application backend. Cloudflare Workers Static Assets hosts the SPA as a single static deployment.

Do not add a Worker API, tRPC, Effect, D1, R2, KV, Queues, or Durable Objects for the PoC. Add a TypeScript Worker API only when authenticated persistence or sharing becomes an accepted product requirement.

### Analyzer

Implement the analyzer and CLI as a native Rust binary.

- Use Oxc to parse TypeScript and resolve scopes, symbols, and modules required by the supported NestJS recognizers.
- Use Serde to emit the normalized graph as JSON; do not expose Oxc-specific AST structures.
- Treat unresolved or unsupported constructs as explicit diagnostics instead of inventing relationships.

Do not rely only on regex parsing.

Do not add a TypeScript analyzer fallback. There should be one canonical analysis path.

Optional later:

- Tree-sitter for multi-language support
- LSP for deeper semantic relationships

## Architecture decision — PoC technology stack

**Date**: 2026-09-05  
**Status**: accepted

### Context

CodebaseCanvas is local-first, analyzes potentially sensitive repositories, and must remain responsive on large graphs. The PoC requires neither server rendering nor cloud persistence.

### Decision

Use a native Rust/Oxc analyzer and a React/Vite/Cytoscape.js SPA hosted as Cloudflare Workers Static Assets. Keep `graph.json` local and use it as the only boundary between the analyzer and UI.

### Alternatives considered

- **TypeScript Compiler API** offers the canonical TypeScript type checker, but requires a Node.js analyzer and does not meet the chosen native Rust CLI direction. If Oxc fails the fixture accuracy gate, stop and reconsider the analyzer language before implementation rather than adding a fallback.
- **Next.js** adds server and rendering capabilities that the PoC does not use. Vite maps directly to Cloudflare's SPA tooling.
- **React Flow** favors editable node-based interfaces. Cytoscape.js better matches graph traversal, filtering, compound nodes, and future graph algorithms.
- **tRPC** works on Cloudflare Workers but provides no type-safe bridge from the Rust analyzer. Introduce it only for a future TypeScript client-to-Worker API.
- **Effect** overlaps with tRPC in schema, error, and RPC concerns, while the PoC has no asynchronous server workflow. Reconsider it only when a concrete workflow needs typed errors, resource management, or controlled retries.
- **Rust/Wasm analysis on Cloudflare Workers** cannot access the user's repository and adds Worker resource limits. Keep analysis native and local.

### Consequences

- Repository data remains on the user's machine during the PoC.
- The UI deploys as static assets without an application server or database.
- Rust and TypeScript remain decoupled through the normalized JSON contract.
- Oxc accuracy must be proven against NestJS fixtures and a representative real repository before expanding recognizers.

## Analyzer pipeline

```text
Repository
   |
   v
Discover files
   |
   v
Parse TypeScript AST
   |
   v
Resolve symbols
   |
   v
Detect NestJS semantics
   |
   v
Build nodes
   |
   v
Build edges
   |
   v
Normalize graph
   |
   v
Persist/export JSON
```

## Framework recognizers

The analyzer should recognize common NestJS patterns:

- `@Module`
- `@Controller`
- `@Injectable`
- constructor injection
- route decorators:
  - `@Get`
  - `@Post`
  - `@Put`
  - `@Patch`
  - `@Delete`

## Suggested project structure

```text
/apps
  /web
/crates
  /analyzer
/examples
  /nestjs-sample
```

Keep the Rust analyzer and TypeScript UI in one repository, connected only through the normalized JSON graph. Avoid additional services or shared-code scaffolding for the PoC.

## Data flow

The analyzer outputs a normalized graph:

```json
{
  "nodes": [],
  "edges": []
}
```

The UI should not depend directly on AST structures.

The UI validates the selected JSON before rendering it. Invalid or unsupported graph data is rejected with a diagnostic; it is not partially guessed or sent to a server.

This separation is important because future analyzers may support other languages.

## Decision references

- [Oxc parser](https://oxc.rs/docs/guide/usage/parser)
- [Oxc parser architecture](https://oxc.rs/docs/learn/architecture/parser.html)
- [Cloudflare React and Vite guide](https://developers.cloudflare.com/workers/framework-guides/web-apps/react/)
- [Cloudflare Workers Static Assets](https://developers.cloudflare.com/workers/static-assets/)
- [tRPC Fetch and Edge adapter](https://trpc.io/docs/server/adapters/fetch)
- [Effect Platform stability](https://effect.website/docs/v3/platform/introduction)
