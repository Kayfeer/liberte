/**
 * Read-only CDP observation of real MediaStreamTrack wrappers.
 *
 * No capture API, peer connection, IPC or media data is replaced. References are
 * retained so a track cannot disappear from the assertion through garbage
 * collection after the application drops it. This observes JavaScript tracks,
 * not physical hardware or a native CEF device-use counter.
 */
const sessions = new WeakMap();
let nextGroup = 0;

function resultObject(response, operation) {
  if (response?.exceptionDetails) {
    const detail = response.exceptionDetails.exception?.description
      ?? response.exceptionDetails.text ?? 'unknown JavaScript exception';
    throw new Error(`${operation} failed: ${String(detail).slice(0, 500)}`);
  }
  if (!response?.result) throw new Error(`${operation} returned no result`);
  return response.result;
}

async function createState(cdp) {
  const group = `liberte-media-observer-${++nextGroup}`;
  try {
    const prototype = resultObject(await cdp.send('Runtime.evaluate', {
      expression: 'globalThis.MediaStreamTrack?.prototype',
      objectGroup: group,
    }), 'MediaStreamTrack prototype observation');
    if (!prototype.objectId) throw new Error('MediaStreamTrack prototype is unavailable');
    const holder = resultObject(await cdp.send('Runtime.evaluate', {
      expression: '({ tracks: [], captures: new WeakSet() })',
      objectGroup: group,
    }), 'Track reference holder');
    if (!holder.objectId) throw new Error('CDP did not retain the track reference holder');
    return { group, prototype: prototype.objectId, holder: holder.objectId };
  } catch (error) {
    await cdp.send('Runtime.releaseObjectGroup', { objectGroup: group }).catch(() => {});
    throw error;
  }
}

// Executed in the inspected page on a private CDP-held object. Device IDs never
// leave that page: their presence classifies raw capture tracks. Processed Web
// Audio destination tracks and remote WebRTC tracks have no capture device ID.
const snapshotFunction = function (candidates) {
  for (const track of candidates) {
    // queryObjects also sees subclass prototype objects; IDL access brands the
    // actual native wrappers without replacing a browser API.
    let id;
    try { id = track.id; } catch { continue; }
    if (typeof id !== 'string' || !['audio', 'video'].includes(track.kind)) continue;
    if (!this.tracks.includes(track)) this.tracks.push(track);
    const settings = track.getSettings();
    if (typeof settings.deviceId === 'string' && settings.deviceId.length > 0) {
      this.captures.add(track);
    }
  }
  const heldTracks = this.tracks.map((track) => ({
    id: track.id,
    kind: track.kind,
    enabled: track.enabled,
    muted: track.muted,
    readyState: track.readyState,
    captureDevice: this.captures.has(track),
  }));
  const counts = {};
  for (const kind of ['audio', 'video']) {
    const tracks = heldTracks.filter((track) => track.kind === kind);
    const raw = tracks.filter((track) => track.captureDevice);
    counts[kind] = {
      held: tracks.length,
      live: tracks.filter((track) => track.readyState === 'live').length,
      ended: tracks.filter((track) => track.readyState === 'ended').length,
      rawHeld: raw.length,
      rawLive: raw.filter((track) => track.readyState === 'live').length,
      rawEnded: raw.filter((track) => track.readyState === 'ended').length,
    };
  }
  const raw = heldTracks.filter((track) => track.captureDevice);
  return {
    source: 'cdp-media-stream-tracks',
    microphone: counts.audio.rawLive > 0 ? 'active' : 'none',
    camera: counts.video.rawLive > 0 ? 'active' : 'none',
    heldTracks,
    counts,
    rawObserved: { audio: counts.audio.rawHeld, video: counts.video.rawHeld },
    allObservedRawTracksEnded: raw.length > 0 && raw.every((track) => track.readyState === 'ended'),
  };
};

/** Query the actual heap and retain every observed track until releaseCapture. */
export async function observeCapture(cdp) {
  let pending = sessions.get(cdp);
  if (!pending) {
    pending = createState(cdp);
    sessions.set(cdp, pending);
  }
  let state;
  try { state = await pending; }
  catch (error) { sessions.delete(cdp); throw error; }
  const query = await cdp.send('Runtime.queryObjects', {
    prototypeObjectId: state.prototype,
    objectGroup: state.group,
  });
  const objectId = query?.objects?.objectId;
  if (!objectId) throw new Error('Runtime.queryObjects returned no track array');
  try {
    const result = resultObject(await cdp.send('Runtime.callFunctionOn', {
      objectId: state.holder,
      functionDeclaration: snapshotFunction.toString(),
      arguments: [{ objectId }],
      returnByValue: true,
    }), 'Raw track snapshot');
    if (!result.value || result.value.source !== 'cdp-media-stream-tracks') {
      throw new Error('CDP returned an invalid raw track snapshot');
    }
    return result.value;
  } finally {
    await cdp.send('Runtime.releaseObject', { objectId });
  }
}

/** Release only observer references; this never calls track.stop(). */
export async function releaseCapture(cdp) {
  const pending = sessions.get(cdp);
  if (!pending) return;
  sessions.delete(cdp);
  const state = await pending;
  await cdp.send('Runtime.releaseObjectGroup', { objectGroup: state.group });
}
