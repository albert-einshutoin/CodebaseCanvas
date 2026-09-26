import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';

const relationKey = r => `${r.from}\0${r.kind}\0${r.to}`;
const sameMetadata = (expected, actual) => expected == null || Object.entries(expected).every(([key, value]) => actual?.[key] === value);
const hasEvidence = (expected, actual) => actual?.some(e => e.file === expected.file && e.line === expected.line && e.source === expected.source && e.confidence === expected.confidence);
const relevantRelations = item => item.expectedRelations ?? (item.expectedRelation ? [item.expectedRelation] : []);
const rate = (numerator, denominator) => denominator === 0 ? null : numerator / denominator;

export function evaluate(ledgerText, freeze, graph) {
  if (createHash('sha256').update(ledgerText).digest('hex') !== freeze.ledgerSha256) throw Error('freeze hash mismatch');
  const ledger = JSON.parse(ledgerText);
  if (ledger.version !== freeze.version && ledger.version !== 'test') throw Error('freeze version mismatch');
  const items = ledger.items;
  const edges = new Map(graph.edges.map(e => [relationKey(e), e]));
  const nodes = new Map(graph.nodes.map(n => [n.id, n]));
  const diagnostics = graph.diagnostics;
  const diagnosticDuplicates = [];
  const diagnosticWrongScope = [];
  const unexpectedScopedDiagnostics = [];
  const provenanceErrors = [];
  const nodeErrors = [];
  const expectedNodeIds = new Set(items.filter(i => i.expectedNode).map(i => i.expectedNode.id));
  const sourceNodeKinds = new Set(['class', 'interface', 'service', 'controller', 'module', 'repository', 'method', 'endpoint']);
  const unexpectedNodes = graph.nodes.filter(n => sourceNodeKinds.has(n.kind) && !expectedNodeIds.has(n.id)).map(n => ({ id: n.id, kind: n.kind, file: n.file, line: n.line }));
  const seenDiagnostics = new Map();
  for (const d of diagnostics) {
    const key = [d.code, d.file, d.line, d.relatedNodeId, d.skippedCount].join('\0');
    const count = (seenDiagnostics.get(key) ?? 0) + 1;
    seenDiagnostics.set(key, count);
    if (count === 2) diagnosticDuplicates.push({ code: d.code, file: d.file, line: d.line, relatedNodeId: d.relatedNodeId, skippedCount: d.skippedCount });
  }
  for (const item of items.filter(i => i.expectedNode)) {
    const expected = item.expectedNode, actual = nodes.get(expected.id);
    if (!actual || actual.kind !== expected.kind || actual.name !== expected.name || actual.file !== expected.file || actual.line !== expected.line ||
        (expected.parentId !== undefined && actual.parentId !== expected.parentId) ||
        (expected.metadata && !sameMetadata(expected.metadata, actual.metadata))) {
      nodeErrors.push({ itemId: item.itemId, expectedId: expected.id, reason: actual ? 'identity or metadata mismatch' : 'node missing' });
    } else if (!hasEvidence(expected.evidence, actual.evidence)) {
      provenanceErrors.push({ itemId: item.itemId, entity: 'node', id: expected.id, expected: expected.evidence });
    }
  }
  const memberships = new Map();
  for (const item of items.filter(i => i.family === 'module' && i.expectedRelation?.kind === 'contains')) {
    const r = item.expectedRelation;
    if (!memberships.has(r.to)) memberships.set(r.to, new Set());
    memberships.get(r.to).add(r.from);
  }
  for (const item of items.filter(i => i.family === 'declaration' && i.expectedNode?.id.startsWith('class:'))) {
    const expectedParents = memberships.get(item.expectedNode.id) ?? new Set();
    const expected = expectedParents.size === 1 ? [...expectedParents][0] : undefined;
    const actual = nodes.get(item.expectedNode.id)?.parentId;
    if (actual !== expected) nodeErrors.push({ itemId: item.itemId, expectedId: item.expectedNode.id, reason: 'display parent differs from distinct Module memberships', expectedParentId: expected ?? null, actualParentId: actual ?? null });
  }
  const families = {};
  const relationState = r => {
    const actual = edges.get(relationKey(r));
    return actual && sameMetadata(r.metadata, actual.metadata) ? actual : null;
  };
  for (const family of ['module', 'di', 'endpoint']) {
    const sourceItems = items.filter(i => i.family === family && i.scored !== false);
    const expected = new Map();
    for (const item of sourceItems) for (const r of relevantRelations(item)) expected.set(relationKey(r), r);
    const correct = [], missing = [];
    for (const [key, r] of expected) {
      const actual = relationState(r);
      if (actual) {
        correct.push(key);
        if (!hasEvidence(r.evidence, actual.evidence)) provenanceErrors.push({ family, entity: 'edge', key, expected: r.evidence });
      } else missing.push(key);
    }
    const sources = new Set(sourceItems.flatMap(i => relevantRelations(i).map(r => r.from).concat(i.expectedDiagnostic?.scope ?? [])));
    const exposedEndpoints = new Set(graph.edges.filter(e => e.kind === 'exposes' && sources.has(e.from)).map(e => e.to));
    const expectedEndpoints = new Set(sourceItems.flatMap(i => i.expectedRelations?.filter(r => r.kind === 'exposes').map(r => r.to) ?? []));
    const actualFamily = graph.edges.filter(e => family === 'module'
      ? sources.has(e.from) && (e.kind === 'depends_on' || (e.kind === 'contains' && nodes.get(e.to)?.kind !== 'method'))
      : family === 'di' ? sources.has(e.from) && e.kind === 'injects'
        : (sources.has(e.from) && e.kind === 'exposes') || ((exposedEndpoints.has(e.from) || expectedEndpoints.has(e.from)) && e.kind === 'depends_on'));
    const falseRelations = actualFamily.filter(e => !expected.has(relationKey(e)) || !sameMetadata(expected.get(relationKey(e)).metadata, e.metadata)).map(relationKey);
    const unsupported = sourceItems.filter(i => i.status === 'unsupported');
    let diagnosed = 0;
    const diagnosedItems = [];
    for (const item of unsupported) {
      const d = item.expectedDiagnostic;
      if (!d) continue;
      const matches = diagnostics.filter(a => a.code === d.code && a.file === d.file && a.line === d.line);
      const actual = matches.find(a => a.relatedNodeId === d.scope);
      if (actual) {
        diagnosed++;
        diagnosedItems.push({ itemId: item.itemId, code: actual.code, file: actual.file, line: actual.line, relatedNodeId: actual.relatedNodeId });
      }
      else if (matches.length) diagnosticWrongScope.push({ itemId: item.itemId, expectedScope: d.scope, actualScopes: [...new Set(matches.map(a => a.relatedNodeId ?? null))] });
    }
    const unassessed = sourceItems.filter(i => !['supported', 'unsupported'].includes(i.status)).length;
    const unresolvedItems = sourceItems.filter(i => i.status !== 'supported' || relevantRelations(i).some(r => !relationState(r) || !hasEvidence(r.evidence, relationState(r).evidence)) || (i.expectedNode && nodeErrors.some(e => e.itemId === i.itemId))).map(i => i.itemId);
    const silentOmission = unsupported.length - diagnosed;
    const missingRate = rate(missing.length, expected.size);
    const unresolvedRate = rate(unresolvedItems.length, sourceItems.length);
    const explanationRate = rate(diagnosed, unsupported.length);
    // Keep every criterion independent; source-level unknown cannot be diluted by edge volume.
    const exceeds = falseRelations.length > (ledger.criteria?.falseRelationsMax ?? 0) ||
      (missingRate !== null && missingRate > (ledger.criteria?.supportedMissingRateMax ?? 0.05)) ||
      (explanationRate !== null && explanationRate < (ledger.criteria?.unsupportedDiagnosticCoverageMin ?? 1)) ||
      (unresolvedRate !== null && unresolvedRate > (ledger.criteria?.majorStructureUnresolvedRateMax ?? 0.20));
    families[family] = { sourceItems: sourceItems.length, supportedExpected: expected.size, correct: correct.length, missing: missing.length, false: falseRelations.length,
      unsupported: unsupported.length, diagnosed, silentOmission, unassessed, unresolved: unresolvedItems.length, missingRate, explanationRate, unresolvedRate,
      verdict: unassessed || sourceItems.length === 0 ? 'NOT_ASSESSABLE' : exceeds ? 'EXCEEDED' : expected.size === 0 ? 'NOT_ASSESSABLE' : 'WITHIN',
      correctKeys: correct, missingKeys: missing, falseKeys: falseRelations, unresolvedItems, diagnosedItems };
  }
  const expectedScoped = items.filter(i => i.expectedDiagnostic).map(i => i.expectedDiagnostic);
  for (const d of diagnostics.filter(d => /^(unsupported_module_|unsupported_di_|unsupported_route_)/.test(d.code))) {
    if (!expectedScoped.some(e => e.code === d.code && e.file === d.file && e.line === d.line && e.scope === d.relatedNodeId))
      unexpectedScopedDiagnostics.push({ code: d.code, file: d.file, line: d.line, relatedNodeId: d.relatedNodeId });
  }
  const callItems = items.filter(i => i.family === 'call');
  const expectedCalls = new Map(callItems.flatMap(i => relevantRelations(i)).map(r => [relationKey(r), r]));
  const actualCalls = graph.edges.filter(e => e.kind === 'calls');
  const falseCalls = actualCalls.filter(e => !expectedCalls.has(relationKey(e))).map(relationKey);
  const missingCalls = [...expectedCalls.keys()].filter(key => !edges.has(key));
  const emittedSites = callItems.filter(i => i.expectedRelation);
  const missingCallSites = emittedSites.filter(i => !edges.has(relationKey(i.expectedRelation)) || !hasEvidence(i.expectedRelation.evidence, edges.get(relationKey(i.expectedRelation)).evidence)).map(i => i.itemId);
  const unknownByMethod = new Map();
  for (const i of callItems.filter(i => i.expectedUnknown)) {
    const key = i.expectedUnknown.scope;
    if (!unknownByMethod.has(key)) unknownByMethod.set(key, []);
    unknownByMethod.get(key).push(i);
  }
  const groups = diagnostics.filter(d => d.code.startsWith('unsupported_call_'));
  const groupKeys = new Map();
  for (const d of groups) groupKeys.set(`${d.relatedNodeId}\0${d.code}`, (groupKeys.get(`${d.relatedNodeId}\0${d.code}`) ?? 0) + 1);
  const unknownMismatches = [];
  const expectedGroups = ledger.expectedCallUnknownGroups ?? [];
  for (const expected of expectedGroups) {
    const rows = groups.filter(d => d.relatedNodeId === expected.scope && d.code === expected.code);
    if (rows.length !== 1 || rows[0].file !== expected.file || rows[0].line !== expected.representativeLine || rows[0].skippedCount !== expected.skippedCount)
      unknownMismatches.push({ scope: expected.scope, code: expected.code, expectedCount: expected.skippedCount, expectedLine: expected.representativeLine, actual: rows.map(d => ({ file: d.file, line: d.line, skippedCount: d.skippedCount })) });
  }
  for (const d of groups) {
    if (!expectedGroups.some(g => g.scope === d.relatedNodeId && g.code === d.code))
      unknownMismatches.push({ scope: d.relatedNodeId, code: d.code, reason: 'unexpected group', actual: { file: d.file, line: d.line, skippedCount: d.skippedCount } });
  }
  for (const [method, sites] of unknownByMethod) {
    const rows = groups.filter(d => d.relatedNodeId === method);
    if (rows.reduce((n, d) => n + (d.skippedCount ?? 0), 0) !== sites.length)
      unknownMismatches.push({ method, reason: 'method skipped count mismatch', expectedSites: sites.length, actualSites: rows.reduce((n,d)=>n+(d.skippedCount??0),0) });
  }
  const summary = graph.metadata.callAnalysis;
  const expectedSummary = { examinedCalls: callItems.length, emittedCalls: emittedSites.length, skippedCalls: callItems.length - emittedSites.length };
  const calls = { expectedSummary, actualSummary: summary, summaryMatches: summary.scope === 'parsed_named_class_methods' && summary.mode === 'same_class_only' && Object.entries(expectedSummary).every(([k, v]) => summary[k] === v),
    expectedEdges: expectedCalls.size, actualEdges: actualCalls.length, missingEdges: missingCalls, falseEdges: falseCalls, missingSites: missingCallSites,
    unknownMismatches, duplicateGroups: [...groupKeys].filter(([, count]) => count > 1).map(([key, count]) => ({ key, count })),
    skippedDiagnosticSum: groups.reduce((n, d) => n + (d.skippedCount ?? 0), 0), groups: groups.map(d => ({ code: d.code, file: d.file, line: d.line, relatedNodeId: d.relatedNodeId, skippedCount: d.skippedCount })) };
  // Same-line named imports can produce identical wire diagnostics; record candidate duplicates without inferring duplicate emission.
  const familyVerdicts = Object.values(families).map(f => f.verdict);
  const technicalVerdict = familyVerdicts.includes('EXCEEDED') || falseCalls.length || missingCallSites.length || nodeErrors.length || unexpectedNodes.length || provenanceErrors.length ||
    !calls.summaryMatches || calls.skippedDiagnosticSum !== summary.skippedCalls || unknownMismatches.length || calls.duplicateGroups.length || diagnosticWrongScope.length || unexpectedScopedDiagnostics.length
    ? 'EXCEEDED' : familyVerdicts.includes('NOT_ASSESSABLE') ? 'NOT_ASSESSABLE' : 'WITHIN';
  return { ledgerVersion: ledger.version, technicalVerdict, families, calls, nodeErrors, unexpectedNodes, provenanceErrors, diagnosticDuplicates, diagnosticWrongScope, unexpectedScopedDiagnostics };
}


export function verifyInputFiles(ledger, freeze, inputRoot) {
  const expected = new Map(ledger.sourceInventory.map(f => [f.file, f.sha256]));
  const found = [];
  function walk(dir) {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.isFile() && /\.tsx?$/.test(entry.name)) found.push(relative(inputRoot, full).replaceAll('\\', '/'));
    }
  }
  walk(join(inputRoot, 'src'));
  if (found.length !== expected.size || found.some(file => !expected.has(file))) throw Error('input source inventory mismatch');
  const manifest = [...expected].map(([file, sha]) => `${sha}  ${file}\n`).join('');
  if (createHash('sha256').update(manifest).digest('hex') !== freeze.inputSourceManifestSha256) throw Error('input manifest freeze mismatch');
  for (const [file, sha] of expected) {
    if (createHash('sha256').update(readFileSync(join(inputRoot, file))).digest('hex') !== sha) throw Error(`input source hash mismatch: ${file}`);
  }
  return { sourceFiles: expected.size, manifestSha256: freeze.inputSourceManifestSha256 };
}

export function makeResult(ledgerText, freeze, graph, graphText, inputVerification) {
  const provenance = {
    ledgerSha256: freeze.ledgerSha256,
    inputSourceManifestSha256: freeze.inputSourceManifestSha256,
    graphSha256: createHash('sha256').update(graphText).digest('hex'),
    analyzerVersion: graph.metadata.analyzerVersion,
    analyzedAt: graph.metadata.analyzedAt,
    rootName: graph.metadata.rootName,
  };
  return { provenance, inputVerification, ...evaluate(ledgerText, freeze, graph) };
}

if (process.argv[1]?.endsWith('/evaluate-real-repo.mjs')) {
  const [ledgerPath, freezePath, graphPath, inputRoot, outputPath] = process.argv.slice(2);
  if (!ledgerPath || !freezePath || !graphPath || !inputRoot || !outputPath) throw Error('usage: node --experimental-strip-types evaluate-real-repo.mjs LEDGER FREEZE GRAPH INPUT_ROOT OUTPUT');
  const { SystemGraphSchema } = await import('../../apps/web/src/graph.ts');
  const ledgerText = readFileSync(ledgerPath, 'utf8');
  const frozen = JSON.parse(readFileSync(freezePath, 'utf8'));
  const inputVerification = verifyInputFiles(JSON.parse(ledgerText), frozen, inputRoot);
  const graphText = readFileSync(graphPath, 'utf8');
  const graph = SystemGraphSchema.parse(JSON.parse(graphText));
  const result = makeResult(ledgerText, frozen, graph, graphText, inputVerification);
  writeFileSync(outputPath, JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify({ technicalVerdict: result.technicalVerdict, families: Object.fromEntries(Object.entries(result.families).map(([k,v]) => [k,{ sourceItems:v.sourceItems, supportedExpected:v.supportedExpected, correct:v.correct, missing:v.missing, false:v.false, unsupported:v.unsupported, diagnosed:v.diagnosed, silentOmission:v.silentOmission, unresolved:v.unresolved, verdict:v.verdict }])), calls: { expectedSummary: result.calls.expectedSummary, actualSummary: result.calls.actualSummary, missingEdges: result.calls.missingEdges.length, falseEdges: result.calls.falseEdges.length, unknownMismatches: result.calls.unknownMismatches.length } }, null, 2));
}
