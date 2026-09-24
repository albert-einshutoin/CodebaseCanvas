# Current-output browser E2E (#26)

`pnpm web:e2e` is the local Chromium gate for the current checkout. It builds the Rust `codebasecanvas` binary with `--locked`, copies only `examples/nestjs-sample/src`, `prisma`, and `tsconfig.json` into a unique temporary repository, analyzes that copy, reads its newly written `.codebasecanvas/graph.json`, builds the current Web app, and starts a loopback Vite preview through Playwright's `webServer`. The browser selects the original generated JSON through the production file input. No checked-in oracle or previous graph is used as the browser input.

## Preparation and commands

Use Node 24.2.0 or later in the 24.x line, pnpm 11.8.0, and Rust 1.96.0. From the repository root:

```sh
pnpm install --frozen-lockfile --ignore-scripts
pnpm --filter @codebasecanvas/web exec playwright install chromium
pnpm web:e2e
```

On Ubuntu CI, install the package-matched browser and system libraries after the frozen pnpm install with `pnpm --filter @codebasecanvas/web exec playwright install --with-deps chromium`. The pinned development dependency is `@playwright/test@1.63.0`; no global Playwright or `pnpm dlx` is used. The existing `pnpm run ci` and `pnpm run ci-pr` paths include `web:e2e` after Rust, Web, and fixture checks. The existing `Rust / Web quality` job runs this on both pull requests and main pushes. A local run does not establish hosted CI success.

The entry point logs the checkout HEAD and whether it is dirty, the binary path taken from that invocation's Cargo artifact output, CLI exit status, generated file path and SHA-256, node/edge/diagnostic counts, and `analyzedAt`. It does not claim an uncommitted checkout is identified by HEAD alone. Cargo's incremental cache can be reused; the build command still runs every time. Build failure, CLI failure, absent/non-regular/invalid JSON, Web build failure, preview conflict, missing browser, or any failing/empty Playwright run fails the command. There is no saved-graph or alternate-binary fallback. The runner removes its own temporary repository in `finally`; Playwright owns and stops its strict-port preview. It does not remove another worktree, server, or browser installation.

## Assertions and boundaries

The primary browser test fixes AuthService/TokenRepository IDs, roles, source paths, requested-token relationship, and the fixture's 57/90/15 and call-site 9/2/7 counts independently of whatever the analyzer emitted. The generated file supplies only transport-dependent data such as its UTC timestamp. A read-only attribute on `.canvas-viewport` samples **actual Cytoscape elements** after synchronous fCoSE layout, controlled selection, and focus styling. It reports generation, rendered node/edge counts, selected IDs, dimmed/focused counts, and `layoutReady`; the test also observes an actual canvas and nonzero viewport. Rendered projection counts are distinct from full graph totals.

Search chooses the candidate by canonical ID. Details check the requested-token row, direction, separate node/edge evidence, related unknown, and graph-wide call-site scope. Focus compares actual Cytoscape neighbor, ancestor-frame, edge, and dimmed-node IDs against independent fixture expectations, then checks that clearing it preserves selection, query, and a changed kind filter. The test grants Clipboard permissions only to `http://127.0.0.1:4186`, writes a sentinel, clicks the production Copy Context control, reads `navigator.clipboard.readText()`, and checks identity, token, timestamp, scope/unknown, confidence text, and the 32,000-code-point limit. It does not substitute a mock writer or infer human comprehension from a copied string.

A separate provider-override scenario checks OverrideController's requested `UsersService` token, the injected-receiver unknown diagnostic and handler, and absence of a fabricated call edge to MockUsersService; it also checks the copied context. Negative files are copies of that run's generated graph: an unsupported schema version and an otherwise current-schema graph with one dangling edge. The browser must remove stale Canvas/details/copy state and accept the original file again with a new import generation. Node helper tests inject nonzero build/CLI statuses and missing/invalid output, including a stale graph elsewhere; these are separate from the actual CLI/browser success evidence.

Before navigation, each browser test installs a request route. It permits only the document `GET /` and script/stylesheet requests whose same-origin, query-free paths exactly match the current build's HTML asset references. It aborts and records everything else, including POST, XHR/fetch, beacon, and query-bearing requests. WebSocket attempts are separately closed and recorded. A local forbidden POST and an asset-shaped fetch are negative controls proving that the guard detects request attempts. Service workers are disabled in the test context. This observes this Chromium test period and routes, not every possible environment; blocked attempts still fail the test rather than counting as proof the application never tried to send data. Browser/package downloads and loopback static assets are preparation/serving traffic, not graph uploads.

Chromium is the only E2E browser. This gate does not assess real repositories, complete Prisma semantics, human first-use UX, performance, mobile, pixel-perfect layout, runtime DI resolution, or deployment. Its hosted result remains pending until the separate PR delivery step. The evaluation app remains fixed at `e0a139a6c498ddccc7fb0ba18c74c69beb73935f`; this E2E intentionally builds the current checkout instead.
