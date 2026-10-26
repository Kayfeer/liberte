/** Independent SDK publisher inside the third native CEF QA process. Config arrives only on stdin. */
import { createInterface } from 'node:readline';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { stat, realpath } from 'node:fs/promises';
import { fileURLToPath, pathToFileURL } from 'node:url';

const require = createRequire(new URL('../frontend/package.json', import.meta.url));
const frontend = fileURLToPath(new URL('../frontend/', import.meta.url));
let stage = 'configuration';
let server;
let browser;
let page;
let stopRequested = false;
let controlError = false;
let initialReceived = false;
let snapshotRequests = 0;
let resolveInitial;
let rejectInitial;
let resolveStop;
const initial = new Promise((resolve, reject) => { resolveInitial = resolve; rejectInitial = reject; });
const stopped = new Promise(resolve => { resolveStop = resolve; });
const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
const initialDeadline = setTimeout(() => rejectInitial(new Error('Configuration deadline')), 15000);

function requestStop() {
  stopRequested = true;
  resolveStop();
  if (!initialReceived) rejectInitial(new Error('Configuration missing'));
}
input.on('line', line => {
  if (!initialReceived) {
    initialReceived = true;
    clearTimeout(initialDeadline);
    if (line.length > 32768) { rejectInitial(new Error('Configuration too large')); return; }
    try { resolveInitial(JSON.parse(line)); }
    catch { rejectInitial(new Error('Invalid configuration')); }
    return;
  }
  if (line.trim() === 'snapshot' && published && snapshotRequests < 20) {
    snapshotRequests += 1;
    return;
  }
  if (line.trim() !== 'stop') controlError = true;
  requestStop();
});
input.on('close', requestStop);
process.once('SIGTERM', requestStop);
process.once('SIGINT', requestStop);

function active() {
  if (stopRequested) throw new Error('Participant stopped');
}
function validate(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid configuration');
  for (const name of ['url', 'token', 'roomName', 'encryptionKey', 'identity', 'applicationPath'])
    if (typeof value[name] !== 'string' || !value[name]) throw new Error('Missing configuration');
  if (!Number.isInteger(value.cdpPort) || value.cdpPort <= 1024 || value.cdpPort > 65535) throw new Error('Invalid native CDP port');
  const url = new URL(value.url);
  if (!['ws:', 'wss:'].includes(url.protocol) || !['localhost', '127.0.0.1', '[::1]'].includes(url.hostname)
      || url.username || url.password || !/^[0-9a-f]{64}$/i.test(value.encryptionKey)
      || !/^[0-9a-f]{64}$/i.test(value.identity)) throw new Error('Invalid synthetic SFU target');
  return value;
}

async function bounded(promise, milliseconds) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('Operation deadline')), milliseconds);
    })]);
  } finally { clearTimeout(timer); }
}

async function sha256(path) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest('hex');
}
let configuration;
let published = false;
try {
  configuration = validate(await initial);
  active();
  stage = 'fixture-server';
  const vitePath = resolve(dirname(require.resolve('vite/package.json')), 'dist/node/index.js');
  const { createServer } = await import(pathToFileURL(vitePath).href);
  server = await createServer({
    configFile: resolve(frontend, 'vite.config.ts'),
    root: frontend,
    logLevel: 'silent',
    clearScreen: false,
    server: { host: '127.0.0.1', port: 5173, strictPort: true, hmr: false },
  });
  active();
  await server.listen();
  const address = server.httpServer.address();
  if (!address || typeof address === 'string' || address.address !== '127.0.0.1') throw new Error('Fixture must listen on loopback');
  active();
  stage = 'browser';
  const { chromium } = require('@playwright/test');
  const application = await realpath(configuration.applicationPath);
  if (!(await stat(application)).isFile()) throw new Error('The native CEF application must be a real executable file');
  const fixture = {
    runtime: 'CEF',
    applicationSha256: await sha256(application),
    sourceSha256: Object.fromEntries(await Promise.all([
      ['launcher', fileURLToPath(import.meta.url)],
      ['page', resolve(frontend, 'e2e/native-sfu-adversary.html')],
      ['participant', resolve(frontend, 'e2e/native-sfu-adversary.ts')],
    ].map(async ([name, path]) => [name, await sha256(path)]))),
  };
  active();
  // Python owns this third native CEF process and its isolated profile.
  // Attach to its existing bundled window; never launch another browser.
  browser = await chromium.connectOverCDP('http://127.0.0.1:' + configuration.cdpPort, { timeout: 30000 });
  fixture.browserVersion = browser.version();
  active();
  const pages = browser.contexts().flatMap(context => context.pages()).filter(candidate => {
    const url = new URL(candidate.url());
    return ['http:', 'https:'].includes(url.protocol) && url.host === 'tauri.localhost'
      && !url.username && !url.password;
  });
  if (pages.length !== 1) throw new Error('Expected one bundled native CEF window in the third QA process');
  page = pages[0];
  stage = 'fixture-load';
  await page.goto('http://127.0.0.1:' + address.port + '/e2e/native-sfu-adversary.html', { waitUntil: 'domcontentloaded', timeout: 30000 });
  await page.waitForFunction(() => !!window.nativeSfuAdversary, null, { timeout: 30000 });
  active();
  await page.evaluate(value => window.nativeSfuAdversary.configure(value), {
    url: configuration.url, token: configuration.token, roomName: configuration.roomName,
    encryptionKey: configuration.encryptionKey, identity: configuration.identity,
  });
  configuration.token = '';
  configuration.encryptionKey = '';
  stage = 'publication';
  await page.locator('#publish').click({ timeout: 10000 });
  await Promise.race([
    page.waitForFunction(() => ['published', 'failed'].includes(window.nativeSfuAdversary.snapshot().status), null, { timeout: 70000 }),
    stopped.then(() => { throw new Error('Participant stopped before publication'); }),
  ]);
  active();
  const state = await page.evaluate(() => window.nativeSfuAdversary.snapshot());
  if (state.status !== 'published' || state.e2ee !== true || state.audio !== true || state.video !== true) {
    stage = typeof state.phase === 'string' && /^[a-z-]{1,40}$/.test(state.phase) ? state.phase : 'publication';
    throw new Error('Encrypted media proof failed');
  }
  // Every boolean comes from SDK encryption flags and measured outgoing media.
  process.stdout.write(JSON.stringify({ status: 'published', e2ee: state.e2ee, audio: state.audio, video: state.video,
    audioBytes: state.audioBytes, videoBytes: state.videoBytes, encodedFrames: state.encodedFrames, fixture }) + '\n');
  published = true;
  stage = 'waiting';
  let previous = state;
  let audioProgress = Date.now(), videoProgress = Date.now();
  const observationDeadline = Date.now() + 120000;
  while (!stopRequested) {
    const current = await bounded(page.evaluate(() => window.nativeSfuAdversary.sample()), 5000);
    if (current.status !== 'published' || !current.e2ee || !current.audio || !current.video) {
      throw new Error('The encrypted participant stopped publishing during observation');
    }
    if (current.audioBytes > previous.audioBytes) audioProgress = Date.now();
    if (current.videoBytes > previous.videoBytes && current.encodedFrames > previous.encodedFrames) videoProgress = Date.now();
    if (Date.now() - audioProgress > 10000 || Date.now() - videoProgress > 10000 || Date.now() > observationDeadline) {
      throw new Error('Outgoing media progression or observation deadline failed');
    }
    previous = current;
    while (snapshotRequests > 0) {
      snapshotRequests -= 1;
      process.stdout.write(JSON.stringify({ status: 'publishing', e2ee: current.e2ee,
        audioBytes: current.audioBytes, videoBytes: current.videoBytes, encodedFrames: current.encodedFrames }) + '\n');
    }
    await Promise.race([stopped, new Promise(resolve => setTimeout(resolve, 250))]);
  }
  if (controlError) throw new Error('Expected stop control line');
} catch (error) {
  // A normal stop exits the loop without throwing. Never hide a real media
  // failure merely because a stop line arrived while its observation awaited.
  // Never print errors containing URLs, tokens, keys or browser call arguments.
  process.stdout.write(JSON.stringify({ status: 'failed', stage }) + '\n');
  process.stderr.write('Independent SFU participant failed at ' + stage + ' (' + (error instanceof Error ? error.name : 'Error') + ')\n');
  process.exitCode = 1;
} finally {
  clearTimeout(initialDeadline);
  input.close();
  if (configuration) { configuration.token = ''; configuration.encryptionKey = ''; }
  try {
    if (published && (!page || page.isClosed())) throw new Error('Published participant closed before cleanup proof');
    if (page && !page.isClosed()) {
      const cleanup = await bounded(page.evaluate(async () => {
        const previous = window.nativeSfuAdversary?.snapshot();
        await window.nativeSfuAdversary?.stop();
        return { ...window.nativeSfuAdversary?.snapshot(), wasPublishing: previous?.status === 'published' && previous?.e2ee === true };
      }), 5000);
      if (published && (cleanup?.localEnded !== true || cleanup?.wasPublishing !== true)) throw new Error('Synthetic tracks did not stop');
    }
  } catch {
    if (published) { process.stderr.write('Independent SFU participant cleanup failed\n'); process.exitCode = 1; }
  }
  await bounded(browser?.close(), 5000).catch(() => { process.exitCode = 1; });
  // This disconnects CDP only. Python closes its owned CEF process group next.
  await bounded(server?.close(), 5000).catch(() => { process.exitCode = 1; });
}
