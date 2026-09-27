# Experimental Graph 0.2 repository request contract

**EXPERIMENTAL_CONTRACT / NOT_IN_PRODUCTION.** This document defines only the
`typeorm_repository_request` wire record for Issue #30. The authoritative
production Graph contract, CLI output, File import, and viewer remain v0.1 in
[DATA_MODEL.md](DATA_MODEL.md). The broader TypeORM proposal remains
**PROPOSED / NOT_IMPLEMENTED**. Passing this contract does not prove source
extraction, an installed dependency profile, a provider, or runtime injection.

## Envelope and strictness

The separate experimental reader accepts exactly `schemaVersion: "0.2"`, the
unchanged v0.1 `metadata`, `nodes`, `edges`, and `diagnostics` shapes and
semantics, and a **required** `frameworkDeclarations` array (`[]` is valid).
The graph body's existing identity, references, parent, Endpoint, call coverage,
path, Unicode, metadata depth and safe number rules still apply. The only
accepted declaration kind is `typeorm_repository_request`; another proposed
kind is an error. Every defined object is closed to unknown fields. Optional
fields are omitted; explicit `null` is invalid. No v0.2 object is downgraded,
stripped, or passed to a v0.1 reader. Existing diagnostics keep their v0.1
shape: in particular `relatedNodeId` must identify a node, not a declaration.

The Rust `SystemGraphV02` `from_json` / `validate` / `to_json` and the Web
`SystemGraphV02Schema` are **explicit experimental entry points**. The shared
hand-authored full-graph cases in `contracts/v02-repository-request.json` are
the wire oracle for both implementations. These cases are not Analyzer output.

## One declaration record

| Field | Required shape and relation |
|---|---|
| `id` | `canonical_id("typeorm_decl", [site.file, "typeorm_repository_request", decimal(site.startByte), "parameter:" + decimal(site.parameterIndex)])`. Full kind, lowercase UTF-8 hex, canonical integer spelling. Unique in the array. |
| `kind` | Exactly `typeorm_repository_request`. |
| `ownerId` | Existing source node of kind `module`, `controller`, `service`, `repository`, or `class`; its `file` equals `site.file`. A Method, Endpoint, Interface, database model, or external dependency cannot own it. |
| `site` | Required `file` (normalized repository-relative path), `startByte`, `endByte` (zero-based UTF-8 byte offsets), `line` (one-based), `parameterIndex` (zero-based). All numbers are safe integers; `endByte > startByte`. These are producer assertions: a graph-only validator cannot prove the decorator occupies those source bytes. |
| `origin` | Required `specifier: "@nestjs/typeorm"`, `exportedName: "InjectRepository"`, nonempty `localName` (the binding used at this site), and optional nonempty `lockedVersion`. The version is omitted when unknown; it is not inferred from the specifier. |
| `entityRef` | Optional `{ id, kind, name }`. It must match an existing source class-like node's ID, kind and name. The node has a source file. Omission means no source entity is claimed; a fabricated ID is invalid. Derived tokens require a node of kind `class`. |
| `connection` | Tagged union: `{syntax:"omitted", normalizedName:"default"}`; `{syntax:"literal", value, normalizedName}` with nonempty scalar `value` and identical `normalizedName` (including explicit `"default"`); or `{syntax:"unknown_expression"}` with no name. No empty literal or implicit default for unknown expressions. |
| `token` | Tagged union described below. |
| `derivation` | Required only for derived token. Closed fixed-profile proof descriptor below; absent for unknown token. |
| `evidence` | Nonempty claim-specific array. Exactly one confirmed `{claim:"decorator_origin", source:"resolver", file:site.file, line:site.line, confidence:"confirmed"}` is required. `entityRef` additionally requires exactly one confirmed `argument_reference` from `resolver` at the same file/line; without an entity reference that claim is absent. Optional `endLine` follows existing Evidence position rules. Duplicate/other claims are invalid. This confirms only the stated source claims. |
| `runtime` | Required `{providerExistence:"unverified", visibility:"unverified", injection:"unverified"}`. |

`token` is either `{state:"derived_under_profile", kind:"string", value,
comparisonKey}` or `{state:"unknown", reason}`. The unknown reason is one of
`connection_expression_unknown`, `entity_unresolved`, `profile_unknown`, or
`class_shape_unknown`. It has no value, comparison key, or derivation. A
connection expression that is unknown uses the first reason; otherwise an
absent entity uses the second; otherwise a missing/nonmatching locked version
uses the third. The last reason records an unproved ordinary-class shape even
when the graph references a class. A derived token needs an entity `class`, a
known connection, `origin.lockedVersion: "7.0.0"`, and the fixed derivation.
For default connection its value is `entityRef.name + "Repository"`; for a
named connection it is `connection.normalizedName + "_" + entityRef.name +
"Repository"`. `comparisonKey` is `canonical_id("nest_string", [value])`.
Equal token strings have equal comparison keys even for different source
entities; declarations remain separate occurrences and are never deduplicated.

The only accepted derivation is:

```json
{
  "profile": "@nestjs/typeorm@7.0.0+typeorm@0.2.24/ordinary_class",
  "rule": "@nestjs/typeorm@7.0.0/getRepositoryToken:ordinary_class",
  "sourceSha256": "8395dc1f59d3471724312c4053cbd7af0dff38fd354d0720580b72452af633f9",
  "dependencyProfile": "producer_verified",
  "entityShape": "resolved_local_non_inherited"
}
```

The validator checks every literal, the corresponding origin version, entity
kind, connection state, derived value, and comparison key. A nonempty freeform
`conditions` list is not a substitute. `producer_verified` and `entityShape`
are assertions in input JSON, **not** proof that this validator read a
`package.json`, lockfile, tsconfig, source file, or real installation. Those
checks and source recognition belong to a later producer. A mismatched or
unknown profile cannot be accepted as derived.

## Compatibility and exclusions

The v0.1 reader rejects `frameworkDeclarations` and v0.2; production File
input and CLI stay on v0.1. This experimental reader does not create Graph
edges, diagnostics referring to declaration IDs, provider matches, or viewer
content. It does not accept `typeorm_for_feature`, `typeorm_for_root`, or
`typeorm_external_class_request`; class-token descriptors and C18/C22/C23
metadata/`@Inject` source decisions remain outside this contract. Proposed
fragment examples in `docs/design/` supplied field ideas, but lacked the full
Graph envelope and included diagnostic extensions. The shared cases explicitly
add v0.1 metadata/nodes/edges/diagnostics and use typed `token.reason` for
unknowns. Their successful validation is separate from extraction accuracy.
