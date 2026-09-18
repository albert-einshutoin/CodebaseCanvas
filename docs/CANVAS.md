# Canvas grouping and explicit method LOD

The input is a validated, immutable `SystemGraph`. Canvas coordinates, selection and expansion never write back to its IDs, parentId, metadata, evidence, diagnostics or call counters. The hand-authored NestJS fixture is a UI input, not evidence that the current Analyzer emits a complete graph; production analyze remains nonzero/no-write until #17.

## Projection and state

`canvasState.ts` keeps `selectedNodeId`, `expandedOwnerId` (zero or one) and the import generation separate. Selection or zoom never expands a class. Show methods accepts only a module/controller/service/repository/class with direct methods verified by both parentId and contains. The UI reports counts **in this snapshot**, and disables Show for zero methods. The expanded owner's name and Hide control remain available when another node or a method is selected.

Selecting B while A is expanded keeps A expanded. Show on B switches owners. Hide or switching owners moves a selected disappearing method back to its previous owning class; other selected nodes stay selected. A new import generation resets selection and expansion even for matching IDs. The File input currently unmounts Canvas while loading and mounts it after successful validation. Invalid input renders an error, not the previous graph.

`graphToCytoscapeElements` is a pure projection. Default fixture: 40 nodes / 56 edges (file totals: 57 / 90). Expansion adds only the owner's method IDs, then retains every original edge whose endpoints are visible, including method contains/imports/calls and Endpoint→handler depends_on. It never invents or aggregates edges. Internal injects and its direction remain unchanged; its display label is Requests token.

Optional candidate IDs are a pure-test/consumer boundary, not a filter UI. Required ancestors are retained. Shared UsersService remains one node with no parent and its two canonical memberships; filtering one Module never reassigns it. Endpoints stay inside their Controller; expanded methods inside their owner. External/interface/DB nodes do not acquire proximity-based membership.

## Layout choice and configuration

Compared the previous breadthfirst view and built-in CoSE on the same fixture/viewport. CoSE with both existing and randomized initial positions stretched the Controller/Endpoint bounds excessively. Omitting ancestor-to-child layout forces also failed to improve this, so that experiment was discarded. Final rendering and layout both receive the complete projected canonical edge set.

Selected [fCoSE](https://github.com/iVis-at-Bilkent/cytoscape.js-fcose) 2.2.0 (MIT), whose Cytoscape peer requirement ^3.2.0 includes the existing 3.34.3. Added only this layout plugin, its cose-base/layout-base dependencies and @types/cytoscape-fcose 2.2.5. Exact direct versions and resolved transitive versions are in pnpm-lock.yaml; no optional layout-utilities extension or layout switcher.

Settings live in `canvasLayout.ts`: quality proof, animate false, fit false, nodeDimensionsIncludeLabels true, idealEdgeLength 100, nodeRepulsion 8000, numIter 1000, tile true, tiling padding 35, packComponents false. Initial import uses randomize true and explicit initial Fit (padding 40). Show/Hide uses randomize false and existing positions without automatic Fit. Placement axes are not dependency direction or execution order; follow arrows and relation names.

Layout runs synchronously with animate false, so there is no deferred layout completion/fit callback to apply to another graph. Selection, hover, background clear and parent rerenders do not invoke layout. Show/Hide diffs IDs, removes only hidden elements, adds only new elements, and seeds new methods near the pre-expansion owner position. Existing elements and viewport are retained; necessary force recalculation can still move nodes and take them outside the viewport. Explicit Fit is available. No position persistence or pixel-determinism guarantee.

Canvas suppresses selection callbacks during structural updates and controlled selection synchronization, preventing remove/unselect from overwriting the collapse selection. Ordinary node selection enforces one highlighted node even with modifier keys. Instance destruction also disposes listeners; ResizeObserver is disconnected on unmount.

## Style and limits

Leaves have bounded 155×64 dimensions, 14px kind/name labels and wrapping; full canonical names remain unchanged. Compound nodes get rectangular shapes after role styles, 28px padding and 18px headings; Module headings use 24px and stronger borders. Non-deprecated numeric leaf dimensions replace width/height:label. Edge labels appear for edges adjacent to a selected node or while hovered, including Requests token; edges themselves are not removed to reduce label density.

Dense crossings, long compound headings and full-fit label size are still layout limitations. Zoom/pan and explicit selection help inspect them. This is not a details/evidence panel, filter/search UI or complete unknown view. No bundle-warning threshold change or unmeasured code splitting.

## Recheck

1. `pnpm --filter @codebasecanvas/web test src/canvas.test.ts src/canvasState.test.ts src/canvasLayout.test.ts` and `pnpm web:typecheck`.
2. Start `pnpm web:dev` and use the actual File chooser for `examples/nestjs-sample/expected-graph.json`.
3. Inspect Module frames and outside nodes. Select AuthController (inside AuthModule), read Requests token on its adjacent edge, then Show/Hide. Select its login method and Hide: selection returns to AuthController. Select UsersService while AuthController is expanded: only Show switches expansion.
4. Check Shift selection, background clear, pan/wheel/+/-/Fit/resize. Import another valid graph while expanded, then the same fixture again; state resets. Check same-file reselect, valid empty/one-node graphs and invalid JSON/schema/reference rejection.
5. A validated disposable 220-node/200-edge synthetic graph (20 Modules, 100 classes, 100 methods; initial visible 120/100) was loaded once and selection/Show/pan/Fit continued. FileChooser setFiles→loaded-heading observation was 134 ms including automation overhead; it is neither layout profiling nor a formal #27 performance result.

Browser verification used Codex In-app Browser, 1280×1000 and 800×900 resize. Pinch and callback invocation counts are unmeasured. Selection stability is observed visually and the selection effect has no layout call; not asserted using pixel snapshots. Same-mount prop replacement is inspected statically, separately from the File input's remount path. No initial-user UX study, real-repository accuracy/performance assessment, #26 E2E or #30 formal UX evaluation was performed. Screenshots/logs are kept outside the repository.
