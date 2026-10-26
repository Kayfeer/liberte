/** QA-only W3C subset over CDP. Attaches to the real CEF app; never launches a browser. */
import http from 'node:http';
import { randomUUID } from 'node:crypto';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';

export const ELEMENT_KEY = 'element-6066-11e4-a52e-4f735466cecf';
export class ProtocolError extends Error {
  constructor(code, message) { super(message); this.code = code; }
}
export function bundledOrigin(value) {
  try {
    const url = new URL(value);
    return ['http:', 'https:'].includes(url.protocol) && url.hostname === 'tauri.localhost'
      && !url.port && !url.username && !url.password;
  } catch { return false; }
}
export function keySegments(text) {
  const keys = { '\uE003': 'Backspace', '\uE004': 'Tab', '\uE006': 'Enter', '\uE007': 'Enter',
    '\uE00C': 'Escape', '\uE012': 'ArrowLeft', '\uE013': 'ArrowUp', '\uE014': 'ArrowRight',
    '\uE015': 'ArrowDown', '\uE017': 'Delete' };
  const segments = [];
  let plain = '';
  for (const character of text) {
    if (keys[character]) {
      if (plain) segments.push({ text: plain });
      plain = '';
      segments.push({ key: keys[character] });
    } else if (/^[\uE000-\uF8FF]$/.test(character)) {
      throw new ProtocolError('invalid argument', 'Unsupported WebDriver key');
    } else { plain += character; }
  }
  if (plain) segments.push({ text: plain });
  return segments;
}
export class CdpSession {
  constructor(browser, page) {
    this.browser = browser;
    this.page = page;
    this.id = randomUUID();
    this.elements = new Map();
    this.timeouts = { script: 10000, pageLoad: 30000, implicit: 0 };
  }
  remember(handle) {
    const id = randomUUID();
    this.elements.set(id, handle);
    return { [ELEMENT_KEY]: id };
  }
  async element(id) {
    const handle = this.elements.get(id);
    if (!handle) throw new ProtocolError('stale element reference', 'Unknown or expired element');
    let connected;
    try { connected = await handle.evaluate(node => node.isConnected); }
    catch (error) {
      if (/context was destroyed|JSHandle is disposed|Cannot find context/i.test(error.message)) connected = false;
      else throw error;
    }
    if (!connected) throw new ProtocolError('stale element reference', 'Element is detached');
    return handle;
  }
  async arguments(value) {
    if (Array.isArray(value)) return Promise.all(value.map(item => this.arguments(item)));
    if (value && typeof value === 'object') {
      if (Object.hasOwn(value, ELEMENT_KEY)) return this.element(value[ELEMENT_KEY]);
      return Object.fromEntries(await Promise.all(Object.entries(value).map(async ([key, item]) => [key, await this.arguments(item)])));
    }
    return value;
  }
  async closeCapture() {
    if (!this.captureCdp) return;
    const { releaseCapture } = await import('./cdp_media_probe.mjs');
    try { await releaseCapture(this.captureCdp); }
    finally { await this.captureCdp.detach(); this.captureCdp = undefined; }
  }
  async command(method, path, data) {
    if (method === 'DELETE' && path === '') {
      try { await this.closeCapture(); }
      finally { await this.browser.close(); }
      return null;
    }
    if (this.page.isClosed() || !this.browser.isConnected()) throw new ProtocolError('no such window', 'The native CEF window closed');
    if (!bundledOrigin(this.page.url())) throw new ProtocolError('unknown error', 'Native window left the bundled frontend origin');
    if (method === 'GET' && path === '/url') return this.page.url();
    if (method === 'GET' && path === '/screenshot') return (await this.page.screenshot({ type: 'png', timeout: 15000 })).toString('base64');
    if (method === 'POST' && path === '/timeouts') {
      for (const [key, value] of Object.entries(data)) {
        if (!Object.hasOwn(this.timeouts, key) || !Number.isInteger(value) || value < 0 || value > 120000)
          throw new ProtocolError('invalid argument', 'Unsupported timeout');
        this.timeouts[key] = value;
      }
      return null;
    }
    if (method === 'POST' && ['/element', '/elements'].includes(path)) {
      if (!['css selector', 'xpath'].includes(data.using) || typeof data.value !== 'string')
        throw new ProtocolError('invalid argument', 'Expected a CSS or XPath selector');
      const selector = data.using === 'xpath' ? 'xpath=' + data.value : data.value;
      if (path === '/elements') return (await this.page.$$(selector)).map(handle => this.remember(handle));
      const handle = await this.page.$(selector);
      if (!handle) throw new ProtocolError('no such element', 'Element was not found');
      return this.remember(handle);
    }
    const elementPath = path.match(/^\/element\/([^/]+)\/(click|value|text|property\/value)$/);
    if (elementPath) {
      const handle = await this.element(decodeURIComponent(elementPath[1]));
      const operation = elementPath[2];
      if (method === 'GET' && operation === 'text') return handle.innerText();
      if (method === 'GET' && operation === 'property/value') return handle.evaluate(node => node.value ?? null);
      if (method === 'POST' && operation === 'click') {
        await handle.click({ timeout: 10000 }); // Trusted input; no DOM click or force.
        return null;
      }
      if (method === 'POST' && operation === 'value') {
        if (typeof data.text !== 'string') throw new ProtocolError('invalid argument', 'Expected text');
        const segments = keySegments(data.text); // Validate before any input is dispatched.
        for (const segment of segments) {
          if (segment.key) await handle.press(segment.key, { timeout: 10000 });
          else await handle.type(segment.text, { timeout: 10000 });
        }
        return null;
      }
    }
    if (method === 'POST' && ['/execute/sync', '/execute/async'].includes(path)) {
      if (typeof data.script !== 'string' || !Array.isArray(data.args))
        throw new ProtocolError('invalid argument', 'Expected script and args');
      const args = await this.arguments(data.args);
      // The supplied function body is compiled through CDP, not eval inside the app's CSP.
      const invocation = '(function(){\n' + data.script + '\n}).apply(window, '
        + (path.endsWith('/async') ? '[...args, done])' : 'args)');
      const execution = path.endsWith('/async') ? invocation + ';' : 'Promise.resolve(' + invocation + ').then(done, fail);';
      const expression = '({args, timeout}) => new Promise((resolve, reject) => {'
        + 'const timer = setTimeout(() => reject(new Error("LIBERTE_SCRIPT_TIMEOUT")), timeout);'
        + 'const done = value => { clearTimeout(timer); resolve(value === undefined ? null : value); };'
        + 'const fail = error => { clearTimeout(timer); reject(error); };'
        + 'try { ' + execution + ' } catch (error) { fail(error); } })';
      try {
        // Playwright treats string expressions as values, not callable functions.
        // Compile only in the Node adapter so the renderer receives a real function.
        const callable = new Function('return (' + expression + ')')();
        return (await this.page.evaluate(callable, { args, timeout: this.timeouts.script })) ?? null;
      } catch (error) {
        if (/LIBERTE_SCRIPT_TIMEOUT/.test(error.message)) throw new ProtocolError('script timeout', 'Script did not complete before its deadline');
        throw new ProtocolError('javascript error', error.message);
      }
    }
    if (method === 'POST' && path === '/refresh') {
      await this.closeCapture();
      this.elements.clear();
      await this.page.reload({ waitUntil: 'domcontentloaded', timeout: this.timeouts.pageLoad });
      return null;
    }
    if (method === 'POST' && path === '/liberte/capture-state') {
      const { observeCapture } = await import('./cdp_media_probe.mjs');
      if (!this.captureCdp) this.captureCdp = await this.page.context().newCDPSession(this.page);
      return observeCapture(this.captureCdp);
    }
    if (method === 'POST' && path === '/liberte/permissions') {
      if (!['granted', 'denied', 'prompt'].includes(data.state)) throw new ProtocolError('invalid argument', 'Invalid media permission state');
      const cdp = await this.page.context().newCDPSession(this.page);
      try {
        for (const name of ['microphone', 'camera']) {
          await cdp.send('Browser.setPermission', { permission: { name }, setting: data.state, origin: new URL(this.page.url()).origin });
        }
      } finally { await cdp.detach(); }
      return null;
    }
    throw new ProtocolError('unknown command', 'Unsupported endpoint: ' + method + ' ' + path);
  }
}
async function attach(port) {
  const require = createRequire(new URL('../frontend/package.json', import.meta.url));
  const { chromium } = require('@playwright/test');
  const browser = await chromium.connectOverCDP('http://127.0.0.1:' + port, { timeout: 30000 });
  try {
    const deadline = Date.now() + 15000;
    while (Date.now() < deadline) {
      const pages = browser.contexts().flatMap(context => context.pages()).filter(page => bundledOrigin(page.url()));
      if (pages.length > 1) throw new Error('Expected exactly one bundled native window');
      if (pages.length === 1) return new CdpSession(browser, pages[0]);
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    throw new Error('The native app did not expose its bundled CEF window');
  } catch (error) { await browser.close(); throw error; }
}
export async function serve(port, cdpPort, token) {
  if (!token || token.length < 32) throw new Error('A per-session loopback authorization token is required');
  let session;
  let executing = false;
  const server = http.createServer(async (request, response) => {
    const reply = (status, value) => {
      response.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' });
      response.end(JSON.stringify({ value: value === undefined ? null : value }));
    };
    if (request.headers.authorization !== 'Bearer ' + token || request.headers.origin) {
      reply(403, { error: 'unknown error', message: 'Unauthorized loopback request' }); return;
    }
    if (executing) { reply(409, { error: 'unknown error', message: 'Another command is still executing' }); return; }
    executing = true;
    try {
      const chunks = [];
      let size = 0;
      for await (const chunk of request) {
        size += chunk.length;
        if (size > 4 * 1024 * 1024) throw new ProtocolError('invalid argument', 'Request body too large');
        chunks.push(chunk);
      }
      let data = {};
      if (size) {
        try { data = JSON.parse(Buffer.concat(chunks).toString('utf8')); }
        catch { throw new ProtocolError('invalid argument', 'Malformed JSON'); }
        if (!data || typeof data !== 'object' || Array.isArray(data)) throw new ProtocolError('invalid argument', 'Expected JSON object');
      }
      const path = new URL(request.url, 'http://127.0.0.1').pathname;
      if (request.method === 'GET' && path === '/status') { reply(200, { ready: !session }); return; }
      if (request.method === 'POST' && path === '/session') {
        if (session) throw new ProtocolError('session not created', 'A native session already exists');
        session = await attach(cdpPort);
        reply(200, { sessionId: session.id, capabilities: { browserName: 'cef', platformName: 'linux' } }); return;
      }
      const match = path.match(/^\/session\/([^/]+)(.*)$/);
      if (!match || !session || match[1] !== session.id) throw new ProtocolError('invalid session id', 'Unknown native session');
      const value = await session.command(request.method, match[2], data);
      if (request.method === 'DELETE' && match[2] === '') session = undefined;
      reply(200, value);
    } catch (error) {
      // Preserve failures; never convert driver/script/IPC errors to success.
      reply(500, { error: error.code || 'unknown error', message: String(error.message || error) });
    } finally { executing = false; }
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(port, '127.0.0.1', resolve); });
  return server;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [port, cdpPort] = process.argv.slice(2).map(Number);
  if (![port, cdpPort].every(value => Number.isInteger(value) && value > 1024 && value <= 65535)) throw new Error('Expected loopback bridge and CDP ports');
  await serve(port, cdpPort, process.env.LIBERTE_QA_BRIDGE_TOKEN);
}
