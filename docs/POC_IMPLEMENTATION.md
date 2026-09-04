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
- Detect constructor dependencies
- Detect module composition
- Detect controller routes
- Report unsupported or unresolved constructs without guessing relationships
- Serialize the normalized graph with Serde

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

Suggested grouping:

```text
Module
  Controller
  Service
  Repository
  DB
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

The browser test must prove that the generated fixture graph can be selected locally, validated, rendered, searched, and inspected without a network upload.
