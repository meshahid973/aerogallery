import { test } from 'node:test';
import assert from 'node:assert/strict';
import { checkRelease, isNewerRelease } from '../src/lib/releases.ts';

test('compares stable releases numerically without offering downgrades', () => {
  assert.equal(isNewerRelease('v0.10.0', '0.9.0'), true);
  assert.equal(isNewerRelease('v1.0.0', '1.0.0-rc.1'), true);
  assert.equal(isNewerRelease('1.2.3+build', '1.2.3'), false);
  assert.equal(isNewerRelease('0.1.0', '0.2.0'), false);
  assert.throws(() => isNewerRelease('v1.0.0-rc.1', '0.9.0'));
  assert.throws(() => isNewerRelease('v1.0.0\n', '0.9.0'));
});

test('deduplicates checks, caches successes, handles empty releases and errors', async (t) => {
  const storage = new Map();
  t.mock.method(globalThis, 'fetch', async () => new Response(JSON.stringify({
    tag_name: 'v0.2.0', draft: false, prerelease: false,
  })));
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true, value: {
      getItem: (key) => storage.get(key) ?? null,
      setItem: (key, value) => storage.set(key, value),
    },
  });
  try {
    const first = checkRelease();
    assert.equal(checkRelease(true), first);
    assert.equal((await first).tag, 'v0.2.0');
    assert.equal((await checkRelease()).tag, 'v0.2.0');
    assert.equal(globalThis.fetch.mock.callCount(), 1);
    globalThis.fetch.mock.mockImplementation(async () => new Response(null, { status: 404 }));
    assert.equal((await checkRelease(true)).tag, null);
    globalThis.fetch.mock.mockImplementation(async () => new Response(null, { status: 429 }));
    await assert.rejects(checkRelease(true), /limiting/);
    assert.equal((await checkRelease()).tag, null);
  } finally {
    delete globalThis.localStorage;
  }
});
