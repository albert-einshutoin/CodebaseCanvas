/** Experimental repository-request contract; production File import stays on Graph 0.1. */
import { z } from 'zod';
import { GraphCoreFields, canonicalId, file, graphSemanticError, integer, text } from './graph';

const classKinds = ['module', 'controller', 'service', 'repository', 'class'] as const;
const profile = '@nestjs/typeorm@7.0.0+typeorm@0.2.24/ordinary_class';
const rule = '@nestjs/typeorm@7.0.0/getRepositoryToken:ordinary_class';
const sourceSha256 = '8395dc1f59d3471724312c4053cbd7af0dff38fd354d0720580b72452af633f9';

const SiteSchema = z.strictObject({
  file, startByte: integer, endByte: integer, line: integer.min(1), parameterIndex: integer,
});
const OriginSchema = z.strictObject({
  specifier: z.literal('@nestjs/typeorm'), exportedName: z.literal('InjectRepository'),
  localName: text, lockedVersion: text.optional(),
});
const EntityRefSchema = z.strictObject({ id: text, kind: z.enum(classKinds), name: text });
const ConnectionSchema = z.discriminatedUnion('syntax', [
  z.strictObject({ syntax: z.literal('omitted'), normalizedName: z.literal('default') }),
  z.strictObject({ syntax: z.literal('literal'), value: text, normalizedName: text }),
  z.strictObject({ syntax: z.literal('unknown_expression') }),
]);
const TokenSchema = z.discriminatedUnion('state', [
  z.strictObject({ state: z.literal('derived_under_profile'), kind: z.literal('string'), value: text, comparisonKey: text }),
  z.strictObject({ state: z.literal('unknown'), reason: z.enum(['connection_expression_unknown', 'entity_unresolved', 'profile_unknown', 'class_shape_unknown']) }),
]);
const DerivationSchema = z.strictObject({
  profile: z.literal(profile), rule: z.literal(rule), sourceSha256: z.literal(sourceSha256),
  dependencyProfile: z.literal('producer_verified'), entityShape: z.literal('resolved_local_non_inherited'),
});
const ClaimEvidenceSchema = z.strictObject({
  claim: z.enum(['decorator_origin', 'argument_reference']), source: z.literal('resolver'),
  file, line: integer.min(1), endLine: integer.min(1).optional(), confidence: z.literal('confirmed'),
});
const RepositoryRequestSchema = z.strictObject({
  id: text, kind: z.literal('typeorm_repository_request'), ownerId: text, site: SiteSchema,
  origin: OriginSchema, entityRef: EntityRefSchema.optional(), connection: ConnectionSchema,
  token: TokenSchema, derivation: DerivationSchema.optional(), evidence: z.array(ClaimEvidenceSchema).min(1),
  runtime: z.strictObject({
    providerExistence: z.literal('unverified'), visibility: z.literal('unverified'), injection: z.literal('unverified'),
  }),
});

const WireSchema = z.strictObject({
  schemaVersion: z.literal('0.2'), ...GraphCoreFields,
  frameworkDeclarations: z.array(RepositoryRequestSchema),
});

export const SystemGraphV02Schema = WireSchema.superRefine((graph, ctx) => {
  const fail = (message: string) => ctx.addIssue({ code: 'custom', message });
  const coreError = graphSemanticError(graph);
  if (coreError) { fail(coreError); return; }
  const nodes = new Map(graph.nodes.map(node => [node.id, node]));
  const ids = new Set<string>();
  for (const request of graph.frameworkDeclarations) {
    const { site, origin, entityRef, connection, token, derivation } = request;
    if (ids.has(request.id)) { fail('Duplicate framework declaration ID'); continue; }
    ids.add(request.id);
    const expectedId = canonicalId('typeorm_decl', site.file, 'typeorm_repository_request', String(site.startByte), `parameter:${site.parameterIndex}`);
    const owner = nodes.get(request.ownerId);
    if (request.id !== expectedId || site.endByte <= site.startByte || !owner || !classKinds.includes(owner.kind as typeof classKinds[number]) || owner.file !== site.file) {
      fail(`Invalid repository request site, owner, or ID: ${request.id}`); continue;
    }
    if (entityRef) {
      const target = nodes.get(entityRef.id);
      if (!target || !classKinds.includes(target.kind as typeof classKinds[number]) || !target.file || target.kind !== entityRef.kind || target.name !== entityRef.name) {
        fail(`Invalid entity reference: ${request.id}`); continue;
      }
    }
    const connectionName = connection.syntax === 'unknown_expression' ? undefined : connection.normalizedName;
    if (connection.syntax === 'literal' && connection.value !== connection.normalizedName) {
      fail(`Invalid connection literal: ${request.id}`); continue;
    }
    const expectedReason = connectionName === undefined ? 'connection_expression_unknown'
      : !entityRef ? 'entity_unresolved'
      : origin.lockedVersion !== '7.0.0' ? 'profile_unknown' : undefined;
    if (token.state === 'unknown') {
      if (derivation !== undefined || token.reason !== (expectedReason ?? 'class_shape_unknown')) fail(`Invalid unknown token: ${request.id}`);
    } else {
      const expectedValue = entityRef && connectionName !== undefined
        ? `${connectionName === 'default' ? '' : `${connectionName}_`}${entityRef.name}Repository` : undefined;
      if (expectedReason !== undefined || entityRef?.kind !== 'class' || !derivation || token.value !== expectedValue
        || token.comparisonKey !== canonicalId('nest_string', token.value)) fail(`Invalid derived token: ${request.id}`);
    }
    const claims = new Set<string>();
    for (const item of request.evidence) {
      if (claims.has(item.claim) || item.file !== site.file || item.line !== site.line || (item.endLine !== undefined && item.endLine < item.line)) {
        fail(`Invalid declaration evidence: ${request.id}`); break;
      }
      claims.add(item.claim);
    }
    if (!claims.has('decorator_origin') || claims.has('argument_reference') !== (entityRef !== undefined)) {
      fail(`Missing or unexpected declaration claim: ${request.id}`);
    }
  }
});

export type SystemGraphV02 = z.infer<typeof SystemGraphV02Schema>;
export type RepositoryRequest = SystemGraphV02['frameworkDeclarations'][number];
