/** Transport unit tests only: fake handles/page do not establish native application behavior. */
import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { Readable } from 'node:stream';
import { bundledOrigin, CdpSession, ELEMENT_KEY, keySegments, ProtocolError, serve } from './cdp_driver.mjs';

function protocol(code) {
  return error => error instanceof ProtocolError && error.code === code;
}
function fakeHandle(value = '') {
  const node = { isConnected: true, value };
  const calls = [];
  const handle = {
    node, calls,
    async evaluate(fn) { calls.push(['evaluate']); return fn(node); },
    async click(options) { calls.push(['click', options]); },
    async type(text, options) { calls.push(['type', text, options]); node.value += text; },
    async press(key, options) { calls.push(['press', key, options]); },
    async innerText() { return node.value; },
  };
  return handle;
}
function fixture() {
  const calls = [];
  const handles = new Map();
  const browser = { isConnected: () => true, close: async () => { calls.push(['close']); } };
  const page = {
    isClosed: () => false,
    url: () => 'http://tauri.localhost/index.html',
    async $(selector) { calls.push(['find', selector]); return handles.get(selector) ?? null; },
    async $$(selector) { calls.push(['findAll', selector]); return handles.has(selector) ? [handles.get(selector)] : []; },
    async reload(options) { calls.push(['reload', options]); },
    async evaluate(expression, payload) {
      calls.push(['evaluate', expression, payload]);
      // Emulate only Playwright's argument/result transport, in an isolated JS realm.
      const unwrap = value => {
        if (value?.node && typeof value.evaluate === 'function') return value.node;
        if (Array.isArray(value)) return value.map(unwrap);
        if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, unwrap(item)]));
        return value;
      };
      const evaluated = vm.runInNewContext('(' + String(expression) + ')', { window: {}, setTimeout, clearTimeout });
      // Playwright passes isFunction:false for strings: an arrow-function string
      // is a value, not a function invocation. Preserve that boundary in this fake.
      const result = typeof expression === 'function' ? await evaluated(unwrap(payload)) : await evaluated;
      return result === undefined || typeof result === 'function' ? undefined : JSON.parse(JSON.stringify(result));
    },
  };
  return { browser, page, calls, handles, session: new CdpSession(browser, page) };
}

test('bundled origin accepts only the exact packaged localhost host', () => {
  for (const url of ['http://tauri.localhost/', 'https://tauri.localhost/index.html?route=chat#settings']) assert.equal(bundledOrigin(url), true, url);
  for (const url of [
    'http://localhost:5173', 'http://127.0.0.1:1420', 'http://tauri.localhost:5173',
    'https://tauri.localhost.example.com', 'https://tauri.localhost@evil.example',
    'https://user:password@tauri.localhost/', 'https://tauri.localhost./',
    'file:///tmp/index.html', 'data:text/html,hello', 'tauri://localhost', 'not a URL',
  ]) assert.equal(bundledOrigin(url), false, url);
});

test('leaving the packaged origin prevents scripts and mutations', async () => {
  const { session, page, calls } = fixture();
  page.url = () => 'https://external.example/';
  await assert.rejects(session.command('POST', '/execute/sync', { script: 'return true', args: [] }), protocol('unknown error'));
  assert.deepEqual(calls, []);
});

test('Enter is dispatched through handle.press and ordinary Unicode remains text', async () => {
  const { session } = fixture();
  const handle = fakeHandle('prefix:');
  const ref = session.remember(handle);
  assert.equal(await session.command('POST', `/element/${ref[ELEMENT_KEY]}/value`, { text: 'Salut 👋\n\uE007' }), null);
  assert.deepEqual(handle.calls.filter(([name]) => name !== 'evaluate'), [
    ['type', 'Salut 👋\n', { timeout: 10000 }], ['press', 'Enter', { timeout: 10000 }],
  ]);
  assert.equal(handle.node.value, 'prefix:Salut 👋\n');
});

test('unsupported WebDriver key rejects the whole input before any text is typed', async () => {
  const { session } = fixture();
  const handle = fakeHandle();
  const ref = session.remember(handle);
  await assert.rejects(session.command('POST', `/element/${ref[ELEMENT_KEY]}/value`, { text: 'must not type\uE009' }), protocol('invalid argument'));
  assert.equal(handle.node.value, '');
  assert.deepEqual(handle.calls.map(([name]) => name), ['evaluate']);
  assert.deepEqual(keySegments('\uE006\uE007'), [{ key: 'Enter' }, { key: 'Enter' }]);
});

test('click uses the stored native handle once without a forced or DOM click', async () => {
  const { session } = fixture();
  const handle = fakeHandle();
  const ref = session.remember(handle);
  await session.command('POST', `/element/${ref[ELEMENT_KEY]}/click`, {});
  assert.deepEqual(handle.calls, [['evaluate'], ['click', { timeout: 10000 }]]);
});

test('unknown and detached element references fail without resolving a new selector', async () => {
  const { session, calls } = fixture();
  await assert.rejects(session.element('missing'), protocol('stale element reference'));
  const handle = fakeHandle();
  const ref = session.remember(handle);
  handle.node.isConnected = false;
  await assert.rejects(session.command('POST', `/element/${ref[ELEMENT_KEY]}/click`, {}), protocol('stale element reference'));
  assert.deepEqual(handle.calls, [['evaluate']]);
  assert.deepEqual(calls, []);
});

test('destroyed execution contexts become stale references, while a renderer crash remains a failure', async () => {
  const { session } = fixture();
  const handle = fakeHandle();
  const ref = session.remember(handle);
  handle.evaluate = async () => { throw new Error('Execution context was destroyed'); };
  await assert.rejects(session.element(ref[ELEMENT_KEY]), protocol('stale element reference'));
  handle.evaluate = async () => { throw new Error('Renderer process crashed'); };
  await assert.rejects(session.element(ref[ELEMENT_KEY]), /Renderer process crashed/);
});

test('script arguments resolve W3C references recursively and reject an expired nested reference', async () => {
  const { session } = fixture();
  const handle = fakeHandle('expected');
  const ref = session.remember(handle);
  const payload = [1, { nested: [{ element: ref, unchanged: null }] }];
  const resolved = await session.arguments(payload);
  assert.equal(resolved[1].nested[0].element, handle);
  assert.deepEqual(payload[1].nested[0].element, ref);
  assert.equal(await session.command('POST', '/execute/sync', {
    script: 'return arguments[1].nested[0].element.value;', args: payload,
  }), 'expected');
  handle.node.isConnected = false;
  await assert.rejects(session.arguments(payload), protocol('stale element reference'));
});

test('CSS/XPath find semantics distinguish a missing element from an empty list', async () => {
  const { session, handles, calls } = fixture();
  const handle = fakeHandle();
  handles.set('xpath=//button', handle);
  const found = await session.command('POST', '/element', { using: 'xpath', value: '//button' });
  assert.equal(await session.element(found[ELEMENT_KEY]), handle);
  assert.ok(calls.some(([name, selector]) => name === 'find' && selector === 'xpath=//button'));
  assert.deepEqual(await session.command('POST', '/elements', { using: 'css selector', value: '.missing' }), []);
  await assert.rejects(session.command('POST', '/element', { using: 'css selector', value: '.missing' }), protocol('no such element'));
});

test('timeouts merge partially and refresh invalidates every previous reference', async () => {
  const { session, calls } = fixture();
  const ref = session.remember(fakeHandle());
  await session.command('POST', '/timeouts', { script: 60000 });
  assert.deepEqual(session.timeouts, { script: 60000, pageLoad: 30000, implicit: 0 });
  await session.command('POST', '/refresh', {});
  assert.deepEqual(calls, [['reload', { waitUntil: 'domcontentloaded', timeout: 30000 }]]);
  await assert.rejects(session.element(ref[ELEMENT_KEY]), protocol('stale element reference'));
});

test('async scripts receive a final callback and retain IPC rejection payloads', async () => {
  const { session } = fixture();
  const result = await session.command('POST', '/execute/async', {
    script: 'const done = arguments[arguments.length - 1]; setTimeout(() => done({ok:false,error:arguments[0]}), 1);',
    args: ['permission denied'],
  });
  assert.deepEqual(result, { ok: false, error: 'permission denied' });
  assert.equal(await session.command('POST', '/execute/async', { script: 'arguments[arguments.length - 1]();', args: [] }), null);
});

test('async callback absence times out, and script exceptions remain protocol errors', async () => {
  const { session } = fixture();
  await session.command('POST', '/timeouts', { script: 5 });
  await assert.rejects(session.command('POST', '/execute/async', { script: 'return 123;', args: [] }), protocol('script timeout'));
  await assert.rejects(session.command('POST', '/execute/sync', { script: 'return new Promise(() => {});', args: [] }), protocol('script timeout'));
  await assert.rejects(session.command('POST', '/execute/async', { script: 'throw new Error("boom");', args: [] }), error => protocol('javascript error')(error) && /boom/.test(error.message));
  await assert.rejects(session.command('POST', '/execute/sync', { script: 'throw new Error("boom");', args: [] }), protocol('javascript error'));
});

test('unknown endpoints and closed native windows cannot return a success value', async () => {
  const { session, page } = fixture();
  await assert.rejects(session.command('POST', '/unsupported', {}), protocol('unknown command'));
  page.isClosed = () => true;
  await assert.rejects(session.command('GET', '/url', {}), protocol('no such window'));
});

test('permission control uses CDP only for the actual bundled origin and surfaces denial errors', async () => {
  const { session, page } = fixture();
  const calls = [];
  const cdp = { send: async (...args) => { calls.push(args); }, detach: async () => { calls.push(['detach']); } };
  page.context = () => ({ newCDPSession: async target => { assert.equal(target, page); return cdp; } });
  await session.command('POST', '/liberte/permissions', { state: 'denied' });
  assert.deepEqual(calls, [
    ['Browser.setPermission', { permission: { name: 'microphone' }, setting: 'denied', origin: 'http://tauri.localhost' }],
    ['Browser.setPermission', { permission: { name: 'camera' }, setting: 'denied', origin: 'http://tauri.localhost' }],
    ['detach'],
  ]);
  cdp.send = async () => { throw new Error('Permission API unsupported'); };
  await assert.rejects(session.command('POST', '/liberte/permissions', { state: 'granted' }), /Permission API unsupported/);
  assert.deepEqual(calls.at(-1), ['detach']);
});

test('server routing rejects missing credentials, browser Origin headers and malformed JSON', async () => {
  const token = 'unit-only-token-012345678901234567890123456789';
  const server = await serve(0, 0, token);
  // Exercise the actual routing listener without requiring outbound loopback
  // networking, which is denied in the local executor sandbox.
  const request = (method, path, headers, body = '') => new Promise(resolve => {
    const incoming = Readable.from(body ? [Buffer.from(body)] : []);
    Object.assign(incoming, { method, url: path, headers });
    let status;
    const response = {
      writeHead(code) { status = code; },
      end(bytes) { resolve({ status, body: JSON.parse(bytes) }); },
    };
    server.emit('request', incoming, response);
  });
  try {
    assert.equal(server.address().address, '127.0.0.1');
    for (const headers of [{}, { authorization: `Bearer ${token}`, origin: 'http://tauri.localhost' }]) {
      const response = await request('GET', '/status', headers);
      assert.equal(response.status, 403);
      assert.equal(response.body.value.error, 'unknown error');
    }
    const response = await request('GET', '/status', { authorization: `Bearer ${token}` });
    assert.deepEqual(response.body, { value: { ready: true } });
    const malformed = await request('POST', '/session', { authorization: `Bearer ${token}` }, '{');
    assert.equal(malformed.body.value.error, 'invalid argument');
  } finally {
    await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
  }
});
