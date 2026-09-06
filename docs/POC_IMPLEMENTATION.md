# PoC Implementation Plan

## Milestone 1 — Repository analyzer

Build the repository analyzer as a Rust CLI:

```bash
codebasecanvas analyze ./path/to/repository
```

Output:

```text
.codebasecanvas/graph.json
```

Requirements:

- Before expanding recognizers, pass the #7 capability probe on both the fixture and a representative real NestJS repository pinned to a commit. Record capabilities and unsupported cases; missing required capability stops expansion.
- Discover TypeScript files
- Ignore:
  - node_modules
  - dist
  - build
  - coverage
- Parse source files with Oxc
- Use Oxc semantic analysis and resolver for the supported relationships
- Read relevant TypeScript project configuration
- Detect classes
- Detect interfaces
- Detect NestJS decorators
- Detect requested constructor DI tokens, without claiming resolved provider implementations
- Detect module composition
- Detect controller routes
- Report unsupported or unresolved constructs without guessing relationships
- Serialize the normalized graph with Serde
- Follow [DATA_MODEL.md](DATA_MODEL.md) for edge meanings, membership/display parents, evidence, and mandatory unknown-call diagnostics/counts. Injected-receiver call inference is outside the PoC.

## Milestone 2 — Graph UI

Create a React/Vite SPA using Cytoscape.js. Let the user select a local `graph.json`; do not upload it.

Requirements:

- Render nodes
- Render edges
- Validate the selected graph before rendering
- Pan / zoom
- Search
- Select node
- Details panel
- Dependency highlighting

## Milestone 3 — NestJS-oriented layout

Improve readability.

Grouping follows the contract's display-parent rules. Shared providers remain outside Module groups with all membership edges preserved; DB models remain outside groups.

```text
Module
  Controller
  Service
  Repository
```

Avoid rendering every method by default.

Methods should appear only after zooming in or selecting a class.

## Milestone 4 — Context export

Add:

`Copy Context`

Generate Markdown for a selected node containing:

- role
- dependencies
- dependents
- endpoints
- methods
- file locations

Apply the allowed paths and fixed limits in [DATA_MODEL.md](DATA_MODEL.md#bounded-copy-context): at most 2 hops, 50 items per section, 200 distinct related nodes, and 32,000 Unicode code points. Include requested-token labels, unknown/confidence notices, and deterministic truncation notices. Associated controller endpoints do not prove service execution.

## Milestone 5 — Static deployment

Deploy the SPA with Cloudflare Workers Static Assets using the Cloudflare Vite plugin and Wrangler.

The deployment must contain no Worker API or storage binding.

## Milestone 6 — Health experiment

Optional only if the above works.

Detect one or two basic maintainability signals:

- Circular dependency
- Large class
- High fan-out

Do not build full technical-debt scoring yet.

## Definition of done

The PoC is done when a user can point CodebaseCanvas at a real NestJS repository and understand its main architecture from the generated canvas without first opening the source files.

The analyzer must pass exact structural checks against the fixture project and a representative real repository. Record analysis time and peak memory before making performance claims; Rust adoption alone is not evidence of improved speed or accuracy.

Issue #26 will add a browser test that builds the analyzer from the current checkout and generates a fresh fixture graph in a temporary fixture copy. That exact file must be selected locally, validated, rendered, searched, and inspected without a network upload. Generation/build failure fails the test; a saved JSON graph cannot replace it. Hand-reviewed expected graphs remain the independent structural oracle.

## Contract prerequisites

The milestone descriptions above are product stages; the current dependency order is owned by Epic #1. #3 provides the [wire types, validation and IDs](DATA_MODEL.md#wire-validation-and-ownership), #4 the independently reviewed NestJS fixture, #16 graph assembly, and #19 local import. Follow the shared repository I/O boundary for discovery/config/import/Prisma/output. After source edits, regenerate and re-select the timestamped snapshot.
