# PoC / Minimal MVP Scope

## Scope

The first implementation should be deliberately narrow.

### Supported project type

- TypeScript
- NestJS
- npm / pnpm / yarn repository

### Implementation stack

- Rust with Oxc for repository analysis and the native CLI
- TypeScript with React and Vite for the web UI
- Cytoscape.js for graph rendering and traversal
- JSON as the language-neutral graph boundary
- Cloudflare Workers Static Assets for web hosting

### Runtime boundary

- Repository analysis runs on the user's machine.
- The CLI writes `.codebasecanvas/graph.json` locally.
- The web UI opens and validates that local file in the browser.
- The PoC does not upload source code or graph data to Cloudflare.
- The PoC has no server API, database, or background worker.

### Static analysis targets

Detect:

- Files
- NestJS modules
- Controllers
- Services / Providers
- Injectable classes
- Repositories
- Interfaces
- Classes
- Methods
- Constructor injection
- Imports
- Exports
- Controller routes
- Prisma models if `schema.prisma` exists

### Initial graph node types

- module
- controller
- service
- repository
- class
- interface
- method
- endpoint
- database_model
- external_dependency

### Initial graph edge types

- imports
- injects
- contains
- calls
- exposes
- reads
- writes
- depends_on

Not every edge type must be perfect in the first implementation.

Priority:

1. contains
2. imports
3. injects
4. exposes
5. calls

## UI

### Main screen

A large interactive canvas.

Required:

- Pan
- Zoom
- Fit to screen
- Click node
- Select node
- Highlight direct dependencies
- Hide unrelated nodes
- Search by symbol name

### Node details panel

Display:

- Name
- Type
- File
- Line
- Methods
- Incoming dependencies
- Outgoing dependencies
- Source snippet if easy to support

### Filters

At minimum:

- Module
- Controller
- Service
- Repository
- Database

## Context export

For the selected node, allow:

`Copy Context`

Example:

```text
Component: AuthService
Type: NestJS Service
File: src/auth/auth.service.ts

Dependencies:
- UserRepository
- JwtService
- ConfigService

Used by:
- AuthController

Methods:
- login()
- refresh()
- logout()

Related endpoints:
- POST /auth/login
- POST /auth/refresh
```

The exported context must be plain text or Markdown so the user can paste it into any external LLM.

## Out of scope

Anything not directly required to prove repository-to-visual-system mapping should be postponed.

This includes tRPC, the Effect runtime, Cloudflare D1/R2/Queues, cloud repository analysis, authentication, and persisted graph sharing.
