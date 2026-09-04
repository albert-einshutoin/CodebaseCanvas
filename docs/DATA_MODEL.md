# Graph Data Model

The serialized JSON graph is the canonical contract between the Rust analyzer and the TypeScript web UI. Rust emits it with Serde, and the UI validates it with Zod before rendering. The TypeScript declarations below describe that language-neutral wire format; they are not analyzer implementation types.

## Graph

```ts
interface SystemGraph {
  nodes: GraphNode[];
  edges: GraphEdge[];
}
```

The fixture end-to-end test must fail if the Rust output and TypeScript validator drift apart.

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

  metadata?: Record<string, unknown>;
}
```

## Important principle

A graph relationship should be traceable to evidence where possible.

Future structure:

```ts
interface Evidence {
  source: "ast" | "framework" | "runtime" | "git" | "ci";
  file?: string;
  line?: number;
  confidence?: number;
}
```

The PoC does not need a sophisticated evidence engine, but the schema should not prevent adding one later.

## Stable identifiers

Prefer IDs based on semantic identity rather than array index.

Example:

```text
class:src/auth/auth.service.ts:AuthService
method:src/auth/auth.service.ts:AuthService.login
endpoint:POST:/auth/login
```

This will later make Git diff analysis easier.
