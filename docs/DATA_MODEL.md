# Graph Data Model

The serialized JSON graph is the canonical contract between the Rust analyzer and the TypeScript web UI. Rust emits it with Serde, and the UI validates it with Zod before rendering. The TypeScript declarations below describe that language-neutral wire format; they are not analyzer implementation types.

## Graph

```ts
interface SystemGraph {
  schemaVersion: "0.1";
  metadata: GraphMetadata;
  nodes: GraphNode[];
  edges: GraphEdge[];
  diagnostics: Diagnostic[];
}

interface GraphMetadata {
  analyzerVersion: string;
  analyzedAt: string;
  rootName?: string;
  callAnalysis: {
    scope: "parsed_named_class_methods";
    mode: "same_class_only";
    examinedCalls: number;
    emittedCalls: number;
    skippedCalls: number;
  };
}
```

This document is the canonical v0.1 contract. Issue #26 will add the fixture end-to-end test, which builds the analyzer from the current checkout, generates a fresh graph, and imports that exact file through the production File API and Zod validator. A saved graph cannot substitute for current Rust output.

## Node

```ts
type GraphNodeKind =
  | "module"
  | "controller"
  | "service"
  | "repository"
  | "class"
  | "interface"
  | "method"
  | "endpoint"
  | "database_model"
  | "external_dependency";

interface GraphNode {
  id: string;
  kind: GraphNodeKind;
  name: string;
  qualifiedName?: string;

  file?: string;
  line?: number;
  endLine?: number;

  parentId?: string;

  evidence: Evidence[];
  metadata?: Record<string, unknown>;
}
```

## Edge

```ts
type GraphEdgeKind =
  | "contains"
  | "imports"
  | "injects"
  | "calls"
  | "exposes"
  | "reads"
  | "writes"
  | "depends_on";

interface GraphEdge {
  id: string;
  from: string;
  to: string;
  kind: GraphEdgeKind;

  evidence: Evidence[];
  metadata?: Record<string, unknown>;
}
```

## Evidence and diagnostics

```ts
interface Evidence {
  source: "ast" | "nestjs" | "prisma" | "resolver";
  file: string;
  line?: number;
  endLine?: number;
  confidence: "confirmed" | "best_effort";
}

interface Diagnostic {
  code: string;
  severity: "info" | "warning" | "error";
  message: string;
  file?: string;
  line?: number;
  relatedNodeId?: string;
  skippedCount?: number;
}
```

Each node and edge has at least one source evidence item. `confirmed` refers to the stated static fact, not runtime behavior. Unsupported relationships produce diagnostics, not invented edges. All source paths, including diagnostic paths, are repository-relative with `/` separators; line numbers are 1-based.

### Call coverage

Count each call expression in parsed named class method bodies once, including calls inside nested functions. Framework decorator/metadata expressions outside these bodies are not included. Each examined call site is either emitted or skipped: `examinedCalls = emittedCalls + skippedCalls`. `emittedCalls` counts source sites, not deduplicated edges.

Every skipped site contributes to a diagnostic grouped by containing method and reason, with `relatedNodeId`, file/line, and positive `skippedCount`. The sum of these diagnostic counts equals `metadata.callAnalysis.skippedCalls`. Nested-function, ambiguous, and injected-receiver calls remain unknown. Parse failures carry file diagnostics; zero examined calls does not imply whole-repository coverage. The UI and Copy Context retain the scope and relevant unknown counts.

## Relationship semantics

Class-like means module, controller, service, repository, or class. These are source declarations, not runtime instances.

| Kind | From → to | Meaning |
|---|---|---|
| `contains` | Module → Controller/Service/Repository/Class | Explicit static module registration; many-to-many membership |
| `contains` | Class-like → Method | Lexical method ownership; one owning declaration |
| `imports` | Class-like/Interface/Method → source declaration or external dependency | Imported binding used by this identified consumer; a file-level import alone does not assign a consumer |
| `injects` | NestJS class-like consumer → runtime class token or external class token | Constructor requests this token; **not** the implementation returned by the container |
| `calls` | Method → Method | Statically supported same-class call target only; no inferred runtime execution trace |
| `exposes` | Controller → Endpoint | Controller declares this route |
| `depends_on` | Module → Module | Static module import; does not imply containment |
| `depends_on` | Endpoint → Method | Declared route handler, owned by the exposing Controller |
| `reads` / `writes` | Repository/Service → Database model | Reserved vocabulary; v0.1 validators reject both relations |

Endpoint metadata requires `httpMethod`, normalized `path`, and `controllerMethodId`. Exactly one Controller exposes an endpoint and exactly one handler edge points to that method. Module exports remain resolver findings; they are not represented as `contains` or imports.

### DI token boundary

Every `injects` edge has `metadata.semantics: "requested_token"`. A resolved constructor reference must denote a runtime class value, not an interface or type-only import. Custom `@Inject` arguments and ambiguous tokens remain unsupported with diagnostics.

For `{ provide: UsersService, useClass: MockUsersService }`, a constructor requesting `UsersService` may have a confirmed requested-token edge to `UsersService`; no edge asserts that a `UsersService` implementation executes. UI/context labels must say “Requests token” / “Requested by”. Module visibility, provider overrides, and runtime availability are not proven by this edge.

The PoC does not derive `calls` from DI edges. Injected-receiver calls contribute to unknown diagnostics even if the requested class declares a matching method. Proving provider implementations is outside this PoC.

### Membership and display parents

- Keep one node per source declaration. Role classification changes `kind`, not identity.
- Preserve all explicit Module → declaration membership edges.
- Set a class-like node's `parentId` to its containing Module only when exactly one distinct Module registers it. With zero or multiple memberships, omit it and show the node outside module groups; retain all membership edges in details.
- A Method's parent is its owning declaration; an Endpoint's parent is its exposing Controller. Module import edges never set display parents.
- `parentId` is a display/lexical hierarchy, not a replacement for membership edges. It must exist, cannot refer to itself, and cannot form a cycle.
- UI filtering must preserve necessary ancestor containers or use an explicit flattened view without modifying canonical memberships.

The Rust contract validator and browser semantic validator enforce unique IDs, existing references, the edge endpoint kinds above, parent rules, endpoint metadata/handler agreement, evidence, and call-counter invariants. Invalid contracts are rejected. Unsupported source constructs remain diagnostics; they do not relax graph invariants.

## Bounded Copy Context

Only follow the following paths from the selected node; do not run arbitrary breadth-first traversal:

| Content | Allowed path |
|---|---|
| Direct relationships and methods | One incident edge, preserving kind, direction, and confidence |
| Endpoints associated with a requested class token | Token ← `injects` ← Controller → `exposes` → Endpoint (2 hops) |
| Endpoint handler calls | Endpoint → `depends_on` → Method → `calls` → Method (2 hops) |
| Endpoint controller's requested tokens | Endpoint ← `exposes` ← Controller → `injects` → Token (2 hops) |

Associated endpoints are labeled “Declared on controllers requesting this token”, never “Endpoints executing this service”. Controller DI does not prove that a particular handler uses the token. Handler calls are static relations, not guaranteed runtime execution.

Hard limits: maximum depth 2, 50 items per section, 200 distinct related nodes total, and 32,000 Unicode code points for the complete Markdown. Sort candidates by kind/name/ID (endpoints by HTTP method/path/ID), then truncate deterministically. Do not recursively expand discovered nodes. State omitted counts and reasons. Reserve output space for confidence, scope/unknown notices, and truncation notices; shorten data fields rather than dropping these notices. These are fixed PoC limits, not configuration or token estimates.

## Deterministic identifiers

IDs are opaque strings. `canonical_id(tag, parts)` (Rust) and `canonicalId(tag, ...parts)` (Web) produce `tag:hex(part1):hex(part2):…`. Each component is a **nonempty string encoded as lowercase UTF-8 hexadecimal**, without Unicode normalization. Every colon separates components; punctuation, namespace separators, Unicode, and nested IDs therefore cannot collide. No component contains the local absolute root.

| Node/edge | Tag | Components, in order |
|---|---|---|
| Class-like declaration, regardless of role | `class` | repository-relative file, zero or more lexical scope names, declaration name |
| Interface | `interface` | repository-relative file, zero or more lexical scope names, declaration name |
| Prisma model | `database_model` | repository-relative schema file, model name |
| Method | `method` | owning declaration ID, `instance` or `static`, method name |
| Endpoint | `endpoint` | HTTP method, normalized route path, handler method ID |
| External dependency | `external` | exact import module specifier including subpath, exported name (`default` for default exports) |
| Edge | `edge` | from ID, kind, to ID |

For example, a class named `A` in `src/a.ts` has ID `class:7372632f612e7473:41`. Role changes never change this ID. Qualified display names do not determine identity: lexical scope components do. Overload signatures may share a method only when owner, static/instance status, and name match; merge their evidence. A construct whose lexical identity cannot be established remains unknown, never merged into a guessed declaration. External bindings from different import subpaths remain distinct unless the resolver proves one canonical source declaration.

Endpoint identity includes the handler, so equal HTTP method/path on different handlers remain distinct. `httpMethod` supports GET, POST, PUT, PATCH, DELETE. `path` starts with `/`, is `/` or has no trailing slash, contains no empty, `.` or `..` segments, backslash, whitespace, query, or fragment. Join controller/method route segments and normalize in the recognizer (#11); validators reject unnormalized input.

Validators check ID encoding, role-independent declaration file/name, method owner/static discriminator/name, endpoint metadata, and edge tuple agreement. Empty components and invalid UTF-8 are rejected. External node `name` is its exported name. `qualifiedName` is optional display information. IDs are deterministic within a snapshot; no stability is promised across file moves or refactors.

The builder (#16) merges duplicate edge tuples and evidence before validation. Duplicate node/edge IDs in an imported graph are errors. The Rust `SystemGraph::to_json` validates and orders nodes and edges by ID; evidence and diagnostics use ascending compact Serde JSON strings (declared field order), with exact duplicate evidence removed. Metadata object keys use sorted map order. This is the canonical output order; the Web accepts valid unsorted arrays without rewriting the snapshot. `analyzedAt` is the only expected difference for repeated analysis of identical inputs. Do not include timings or random values elsewhere.

## Wire validation and ownership

- `crates/analyzer/src/lib.rs`: Serde wire types, `SystemGraph::validate`, `from_json`, canonical IDs, and validated deterministic `to_json`. #16 must use these at output boundaries, rather than another validator.
- `apps/web/src/graph.ts`: `SystemGraphSchema` with inferred TypeScript types and canonical IDs. #19 must call `safeParse`/`parse` before installing imported data in application state; failures reject the whole graph.
- `contracts/cases.json`: shared, hand-inspectable positive/negative wire cases and ID vectors consumed by both test runners. These are contract tests, not the NestJS source fixture or proof of extraction accuracy.
- Unknown wire fields outside open `metadata` objects, unsupported versions/enums, explicit null optional fields, empty identifiers/names/messages, malformed paths, invalid references and semantic violations are errors. Optional fields are omitted, not set to null. Metadata values must be JSON values with finite numbers; integer-valued numbers must remain within ±9,007,199,254,740,991 (larger exact values require strings). All metadata keys, including an own `__proto__` key, are preserved without applying them to an object prototype; strings and keys contain Unicode scalar values (no unpaired surrogate). Each open metadata value has maximum nesting depth 32, counting its root at depth 0. This also rejects cyclic in-memory values in the Web validator.
- Counts and line numbers are safe integers at most 9,007,199,254,740,991. Lines start at 1; a line requires a file, and an end line requires a start line and cannot precede it. Counts are nonnegative; `skippedCount` is positive, one diagnostic per method/code. Every emitted call edge requires at least one emitted call site; positive emitted counts require call edges.
- `analyzedAt` is UTC `YYYY-MM-DDTHH:mm:ssZ`, including a valid calendar date. The UI and Copy Context retain this timestamp as snapshot metadata, not proof of synchronization. Source changes require re-analysis and explicit file re-selection.
- `imports` source declarations are class-like/interface/method, and targets are those declarations or external dependencies. Database models and endpoints do not participate in imports. Modules have no display parent; interfaces, database models and external dependencies have none either. Method and endpoint parents are mandatory.

## Repository I/O boundary (implementation in #5, #6, #13)

Discovery, resolver imports, tsconfig references, Prisma schema reads and output writes share one explicitly selected repository root. Wire paths are normalized relative `/` paths with no empty, `.` or `..` components, backslashes, drive/URI colons or control characters. Absolute roots are never serialized. Do not copy raw OS error text containing absolute paths into diagnostic messages; report repository-relative source locations with bounded, sanitized error descriptions. Filesystem access must additionally resolve symlinks and prove containment; a string validator is not a filesystem sandbox.

An unsupported external source reference is not read: mark the affected source unknown with a diagnostic. A boundary violation or unsafe output path is an Error. Do not scan another repository, all modules, or the entire repository as an implicit recovery path. An output error must not report success or overwrite a different destination.

## Downstream acceptance

Issue #4 provides the [NestJS fixture and hand-authored expected graph](../examples/nestjs-sample/README.md) with a shared provider, equal routes with different handlers, `useClass` overrides, and unknown calls. `injects` describes only the requested runtime class token; interface/type-only/custom/ambiguous DI must produce diagnostics in the analyzer, not edges that a shape validator pretends to prove.

Issue #26 must build the current Rust checkout, analyze a disposable copy of that source fixture, and import the newly emitted file through the production File API and this Zod validator. Build/analysis/import failure fails the test; a saved JSON graph cannot substitute for current output. Issue #3 does not implement extraction, File API, GraphBuilder, or this browser E2E.
