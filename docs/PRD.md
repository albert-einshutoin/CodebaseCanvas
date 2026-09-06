# CodebaseCanvas — Minimal PRD

## Problem

AI coding tools dramatically increase implementation speed and code volume.

As a result:

- Humans cannot review all generated code line-by-line.
- Engineers lose an accurate mental model of the system.
- Non-engineers can create applications but cannot understand the generated architecture.
- Duplicate logic, excessive dependencies, large classes, and accidental complexity can accumulate quickly.
- CI failures and bugs become harder to trace as the system grows.

The problem is not that users lack another LLM.

The problem is that software structure is difficult for humans to understand once AI-generated code grows beyond a small size.

## Product thesis

A codebase should be understandable as a visual system, not only as source files.

CodebaseCanvas converts source code into a persistent visual model.

## Target users

### Primary
- Backend engineers
- Tech leads
- Engineering managers
- AI-heavy development teams
- Developers using Codex, Claude Code, Cursor, GitHub Copilot, etc.

### Secondary
- PdMs
- Startup founders
- AI builders / vibe coders
- Non-engineers maintaining AI-generated applications

## Primary job to be done

> When I inherit, generate, or modify a codebase, I want to immediately understand the system structure and dependencies without reading every file.

## PoC goal

Prove that CodebaseCanvas can analyze a TypeScript/NestJS repository and generate a useful interactive system graph.

## PoC success criteria

The PoC is successful if it can:

1. Analyze a real NestJS repository.
2. Detect major framework entities.
3. Generate a dependency graph.
4. Render the graph interactively in a browser.
5. Allow users to inspect source locations from graph nodes.
6. Allow users to focus on one component and its nearby dependencies.
7. Produce a copyable context summary for an external LLM.

## Non-goals for PoC

Do not implement:

- Built-in LLM chat
- Automatic code modification
- Full runtime tracing
- OpenTelemetry integration
- Sentry integration
- GitHub App
- Pull request analysis
- CI log ingestion
- Multi-language support
- Team collaboration
- Authentication
- Billing
- Enterprise security

## Evidence and freshness

A graph describes a static snapshot, with evidence and the `analyzedAt` timestamp defined in [DATA_MODEL.md](DATA_MODEL.md). It does not prove runtime provider execution or current-source synchronization. Unknown relationships remain visible; source changes require re-analysis and file re-selection.
