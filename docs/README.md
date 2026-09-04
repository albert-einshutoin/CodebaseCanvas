# CodebaseCanvas

CodebaseCanvas is a visual software understanding layer for the AI coding era.

AI tools can generate and modify code faster than humans can review and understand it. CodebaseCanvas converts a codebase into a visual system map so developers and non-developers can understand what exists, what changed, and where problems are likely to be.

## Core idea

> AI writes the code. CodebaseCanvas helps humans understand and maintain the system.

The initial product is not an AI coding tool and does not bundle an LLM.

CodebaseCanvas performs deterministic code analysis and presents the result visually. When deeper reasoning is needed, users can copy a structured context package into the LLM or coding agent they already use.

## Initial target

The first PoC/MVP targets:

- TypeScript/NestJS repositories
- A native Rust CLI using Oxc for analysis
- A React/Vite/TypeScript web UI using Cytoscape.js
- Git repositories
- Local repositories first
- A web-based interactive canvas hosted with Cloudflare Workers Static Assets

## Initial user experience

1. User runs the local CLI against a repository.
2. CodebaseCanvas generates `.codebasecanvas/graph.json` locally.
3. User opens that file in the web application; the PoC does not upload it.
4. The application visualizes:
   - Modules
   - Controllers
   - Services
   - Repositories
   - Classes
   - Interfaces
   - Dependencies
   - Endpoints
   - Prisma models where available
5. User can click a node to inspect details.
6. User can trace dependencies and execution paths.
7. User can copy relevant context for use in an external LLM.

## Product direction

Future views may include:

- System Canvas
- Change Canvas
- Failure Canvas
- Health Canvas
- Data Canvas

The PoC should focus only on proving that a real codebase can be converted into a useful, understandable visual model.
