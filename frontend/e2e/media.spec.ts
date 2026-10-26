import { expect, test } from "@playwright/test";
import type { CallSignal } from "../src/lib/mediaSession";
import type {} from "./media-harness";

test("two real WebRTC peers exchange audio/video, mute and release devices", async ({ context }) => {
  const aliceId = "a".repeat(64);
  const bobId = "b".repeat(64);
  const alice = await context.newPage();
  const bob = await context.newPage();
  // Only the encrypted-signalling IPC is substituted. Capture, codecs, ICE,
  // DTLS, peer connections and rendering all execute in Chromium.
  await alice.exposeFunction("transport", async (target: string, signal: CallSignal) => {
    expect(target).toBe(bobId);
    await bob.evaluate(({ sender, signal }) => window.call.receive(sender, signal), { sender: aliceId, signal });
  });
  await bob.exposeFunction("transport", async (target: string, signal: CallSignal) => {
    expect(target).toBe(aliceId);
    await alice.evaluate(({ sender, signal }) => window.call.receive(sender, signal), { sender: bobId, signal });
  });
  await Promise.all([alice.goto("/e2e/media.html"), bob.goto("/e2e/media.html")]);
  await Promise.all([alice.waitForFunction(() => !!window.initializeCall), bob.waitForFunction(() => !!window.initializeCall)]);
  await alice.evaluate((id) => window.initializeCall(id), aliceId);
  await bob.evaluate((id) => window.initializeCall(id), bobId);
  try {
    await Promise.all([alice.evaluate(() => window.call.session.start()), bob.evaluate(() => window.call.session.start())]);
    await bob.evaluate((id) => { window.call.session.ensurePeer(id); }, aliceId);
    await alice.evaluate((id) => { window.call.session.ensurePeer(id); }, bobId);
    await expect.poll(() => alice.evaluate((id) => window.call.state().peers[id]?.state, bobId)).toBe("connected");
    await expect.poll(() => bob.evaluate((id) => window.call.state().peers[id]?.state, aliceId)).toBe("connected");
    for (const page of [alice, bob]) {
      await expect.poll(() => page.evaluate(async () => (await window.call.stats()).some((item) => item.kind === "audio" && item.totalSamplesReceived > 0))).toBe(true);
    }
    await Promise.all([alice.evaluate(() => window.call.session.setVideoEnabled(true)), bob.evaluate(() => window.call.session.setVideoEnabled(true))]);
    await expect.poll(() => alice.evaluate(async () => (await window.call.stats()).some((item) => item.kind === "video" && item.framesDecoded > 0))).toBe(true);
    await expect.poll(() => bob.evaluate(async () => (await window.call.stats()).some((item) => item.kind === "video" && item.framesDecoded > 0))).toBe(true);
    await expect.poll(() => bob.locator("video").evaluate((element) => (element as HTMLVideoElement).videoWidth)).toBeGreaterThan(0);
    await alice.evaluate(() => window.call.session.setMuted(true));
    expect(await alice.evaluate(() => window.call.state().localAudioEnabled)).toBe(false);
    await expect.poll(() => bob.evaluate((id) => window.call.state().peers[id]?.isMuted, aliceId)).toBe(true);
    await alice.evaluate(() => window.call.session.setMuted(false));
    await expect.poll(() => bob.evaluate((id) => window.call.state().peers[id]?.isMuted, aliceId)).toBe(false);
    await alice.evaluate(() => window.call.session.setVideoEnabled(false));
    await expect.poll(() => bob.evaluate((id) => window.call.state().peers[id]?.isVideoEnabled, aliceId)).toBe(false);
    await alice.evaluate(() => window.call.session.stop());
    await expect.poll(() => bob.evaluate(() => Object.keys(window.call.state().peers).length)).toBe(0);
    expect(await alice.evaluate(() => window.call.state().allLocalTracksEnded)).toBe(true);
    await bob.evaluate(() => window.call.session.stop());
    expect(await bob.evaluate(() => window.call.state().allLocalTracksEnded)).toBe(true);
    expect(await alice.evaluate(() => window.call.state().errors)).toEqual([]);
    expect(await bob.evaluate(() => window.call.state().errors)).toEqual([]);
  } finally {
    await Promise.allSettled([alice.evaluate(() => window.call.session.stop()), bob.evaluate(() => window.call.session.stop())]);
  }
});


test("real WebRTC negotiation recovers when the initial signaling publication fails", async ({ context }) => {
  const aliceId = "a".repeat(64);
  const bobId = "b".repeat(64);
  const alice = await context.newPage();
  const bob = await context.newPage();
  let rejectInitialOffer = true;
  await alice.exposeFunction("transport", async (target: string, signal: CallSignal) => {
    expect(target).toBe(bobId);
    if (typeof signal === "object" && "Offer" in signal && rejectInitialOffer) {
      rejectInitialOffer = false;
      throw new Error("Signaling temporarily unavailable");
    }
    await bob.evaluate(({ sender, signal }) => window.call.receive(sender, signal), { sender: aliceId, signal });
  });
  await bob.exposeFunction("transport", async (target: string, signal: CallSignal) => {
    expect(target).toBe(aliceId);
    await alice.evaluate(({ sender, signal }) => window.call.receive(sender, signal), { sender: bobId, signal });
  });
  await Promise.all([alice.goto("/e2e/media.html"), bob.goto("/e2e/media.html")]);
  await Promise.all([alice.waitForFunction(() => !!window.initializeCall), bob.waitForFunction(() => !!window.initializeCall)]);
  await alice.evaluate((id) => window.initializeCall(id), aliceId);
  await bob.evaluate((id) => window.initializeCall(id), bobId);
  try {
    await Promise.all([alice.evaluate(() => window.call.session.start()), bob.evaluate(() => window.call.session.start())]);
    await bob.evaluate((id) => { window.call.session.ensurePeer(id); }, aliceId);
    await alice.evaluate((id) => { window.call.session.ensurePeer(id); }, bobId);
    await expect.poll(() => alice.evaluate(() => window.call.state().errors)).toContain("Signaling temporarily unavailable");
    await alice.evaluate(() => window.call.session.retryConnections());
    await expect.poll(() => alice.evaluate((id) => window.call.state().peers[id]?.state, bobId)).toBe("connected");
    await expect.poll(() => bob.evaluate((id) => window.call.state().peers[id]?.state, aliceId)).toBe("connected");
    for (const page of [alice, bob]) {
      await expect.poll(() => page.evaluate(async () => (await window.call.stats()).some((item) => item.kind === "audio" && item.totalSamplesReceived > 0))).toBe(true);
    }
    expect(await alice.evaluate(() => window.call.state().errors)).toEqual(["Signaling temporarily unavailable"]);
    expect(await bob.evaluate(() => window.call.state().errors)).toEqual([]);
  } finally {
    await Promise.allSettled([alice.evaluate(() => window.call.session.stop()), bob.evaluate(() => window.call.session.stop())]);
  }
});
