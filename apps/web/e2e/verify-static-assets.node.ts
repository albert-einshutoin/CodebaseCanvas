import assert from 'node:assert/strict';
import { test } from 'node:test';
import { assertNoWorkerFeatures } from './verify-static-assets.ts';

test('generated config rejects Worker bindings even when their values are falsy', () => {
  for (const value of [false, 0, '', []]) {
    assert.throws(() => assertNoWorkerFeatures({ vars: { FEATURE: value } }), /binding: vars/);
  }
  assert.throws(() => assertNoWorkerFeatures({ services: [{ binding: 'API', service: 'api' }] }), /binding: services/);
  assert.throws(() => assertNoWorkerFeatures({ main: './worker.ts' }), /entrypoint/);
});
