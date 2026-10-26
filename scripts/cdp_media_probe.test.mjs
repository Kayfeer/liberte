import assert from 'node:assert/strict';
import test from 'node:test';
import vm from 'node:vm';
import { observeCapture, releaseCapture } from './cdp_media_probe.mjs';

class Track {
  constructor(id, kind, device = false) {
    this.id = id;
    this.kind = kind;
    this.device = device;
    this.readyState = 'live';
    this.enabled = true;
    this.muted = false;
  }
  getSettings() { return this.device && this.readyState === 'live' ? { deviceId: 'private-device-id' } : {}; }
}

class ProtocolFixture {
  heap = [];
  objects = new Map();
  calls = [];
  nextId = 0;
  failQuery = false;
  missingPrototype = false;
  context = vm.createContext({ MediaStreamTrack: Track });
  store(value) {
    const objectId = String(++this.nextId);
    this.objects.set(objectId, value);
    return { objectId };
  }
  async send(method, parameters) {
    this.calls.push({ method, parameters });
    if (method === 'Runtime.evaluate') {
      if (this.missingPrototype) return { result: { type: 'undefined' } };
      return { result: this.store(vm.runInContext(parameters.expression, this.context)) };
    }
    if (method === 'Runtime.queryObjects') {
      if (this.failQuery) throw new Error('native wrapper query is unsupported');
      return { objects: this.store([...this.heap]) };
    }
    if (method === 'Runtime.callFunctionOn') {
      const fn = vm.runInContext(`(${parameters.functionDeclaration})`, this.context);
      const value = fn.call(this.objects.get(parameters.objectId), this.objects.get(parameters.arguments[0].objectId));
      return { result: { value: JSON.parse(JSON.stringify(value)) } };
    }
    if (method === 'Runtime.releaseObject') { this.objects.delete(parameters.objectId); return {}; }
    if (method === 'Runtime.releaseObjectGroup') { this.objects.clear(); return {}; }
    throw new Error(`Unexpected CDP method: ${method}`);
  }
}

test('retains real raw track references and cannot pass cleanup without observing capture', async () => {
  const cdp = new ProtocolFixture();
  const initial = await observeCapture(cdp);
  assert.equal(initial.allObservedRawTracksEnded, false);
  const microphone = new Track('raw-mic', 'audio', true);
  const camera = new Track('raw-camera', 'video', true);
  const processed = new Track('web-audio-output', 'audio');
  const remote = new Track('remote-video', 'video');
  cdp.heap = [microphone, camera, processed, remote];
  const active = await observeCapture(cdp);
  assert.equal(active.microphone, 'active');
  assert.equal(active.camera, 'active');
  assert.deepEqual(active.rawObserved, { audio: 1, video: 1 });
  assert.equal(JSON.stringify(active).includes('private-device-id'), false);
  microphone.readyState = 'ended';
  camera.readyState = 'ended';
  cdp.heap = [processed, remote]; // raw objects are no longer returned by a fresh query
  const closed = await observeCapture(cdp);
  assert.equal(closed.allObservedRawTracksEnded, true);
  assert.equal(closed.heldTracks.length, 4);
  assert.equal(closed.counts.audio.rawEnded, 1);
  assert.equal(closed.counts.video.rawEnded, 1);
  assert.equal(closed.camera, 'none');
  assert.equal(closed.microphone, 'none');
  await releaseCapture(cdp);
  assert.equal(cdp.objects.size, 0);
  assert.equal(remote.readyState, 'live'); // observer cleanup never stops application tracks
});

test('missing APIs and rejected native-wrapper queries fail instead of reporting no capture', async () => {
  const absent = new ProtocolFixture();
  absent.missingPrototype = true;
  await assert.rejects(observeCapture(absent), /prototype is unavailable/);
  assert.equal(absent.objects.size, 0);
  const rejected = new ProtocolFixture();
  rejected.failQuery = true;
  await assert.rejects(observeCapture(rejected), /native wrapper query is unsupported/);
  await releaseCapture(rejected);
});
