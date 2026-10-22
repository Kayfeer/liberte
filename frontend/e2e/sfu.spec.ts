import { test, expect, type Page } from "@playwright/test";
import { createHmac, randomBytes } from "node:crypto";

const url = process.env.LIBERTE_SFU_TEST_URL;
const apiKey = process.env.LIBERTE_SFU_API_KEY;
const apiSecret = process.env.LIBERTE_SFU_API_SECRET;

function token(identity: string, room: string): string {
  const encode = (value: unknown) => Buffer.from(JSON.stringify(value)).toString("base64url");
  const now = Math.floor(Date.now() / 1000);
  const body = encode({ alg: "HS256", typ: "JWT" }) + "." + encode({
    iss: apiKey, sub: identity, nbf: now - 5, exp: now + 180,
    video: { roomJoin: true, room, canPublish: true, canSubscribe: true, canPublishData: false },
  });
  return body + "." + createHmac("sha256", apiSecret!).update(body).digest("base64url");
}

interface Harness {
  start(connection: { url: string; token: string; roomName: string; encryptionKey: string }, userId: string): Promise<void>;
  camera(enabled: boolean): Promise<void>;
  audioDecoded(): Promise<boolean>;
  snapshot(): { errors: string[]; frames: { width: number; decoded: number }[]; localEnded: boolean };
  stop(): Promise<void>;
}
declare global { interface Window { sfuHarness: Harness } }

async function start(page: Page, identity: string, roomName: string, encryptionKey: string) {
  await page.goto("/e2e/sfu.html");
  await page.waitForFunction(() => !!window.sfuHarness);
  await page.evaluate(({ connection, identity }) => window.sfuHarness.start(connection, identity), {
    connection: { url: url!, token: token(identity, roomName), roomName, encryptionKey }, identity,
  });
}

test.describe("LiveKit SFU with mandatory E2EE", () => {
  test.skip(!url || !apiKey || !apiSecret, "A local LiveKit service is required; CI supplies it.");

  test("two browsers decrypt video and release capture when leaving", async ({ context }) => {
    const alice = await context.newPage();
    const bob = await context.newPage();
    const room = "liberte-e2ee-" + randomBytes(8).toString("hex");
    const key = randomBytes(32).toString("hex");
    try {
      await start(alice, "a".repeat(64), room, key);
      await start(bob, "b".repeat(64), room, key);
      await alice.evaluate(() => window.sfuHarness.camera(true));
      await bob.evaluate(() => window.sfuHarness.camera(true));
      for (const page of [alice, bob]) {
        await expect.poll(() => page.evaluate(() => window.sfuHarness.audioDecoded())).toBe(true);
        await expect.poll(() => page.evaluate(() => window.sfuHarness.snapshot().frames.some((frame) => frame.width > 0 && frame.decoded > 3))).toBe(true);
        expect(await page.evaluate(() => window.sfuHarness.snapshot().errors)).toEqual([]);
      }
      await alice.evaluate(() => window.sfuHarness.stop());
      await bob.evaluate(() => window.sfuHarness.stop());
      expect(await alice.evaluate(() => window.sfuHarness.snapshot().localEnded)).toBe(true);
      expect(await bob.evaluate(() => window.sfuHarness.snapshot().localEnded)).toBe(true);
    } finally { await alice.close(); await bob.close(); }
  });

  test("a different channel key cannot decode and causes an explicit failure", async ({ context }) => {
    const alice = await context.newPage();
    const bob = await context.newPage();
    const room = "liberte-wrong-key-" + randomBytes(8).toString("hex");
    try {
      await start(alice, "a".repeat(64), room, randomBytes(32).toString("hex"));
      await start(bob, "b".repeat(64), room, randomBytes(32).toString("hex"));
      // A wrong key may already fail on audio, before camera activation.
      await alice.evaluate(() => window.sfuHarness.camera(true).catch(() => {}));
      await expect.poll(async () => {
        const left = await alice.evaluate(() => window.sfuHarness.snapshot());
        const right = await bob.evaluate(() => window.sfuHarness.snapshot());
        return [...left.errors, ...right.errors].some((message) => message.includes("chiffrement"));
      }).toBe(true);
      for (const page of [alice, bob]) {
        const snapshot = await page.evaluate(() => window.sfuHarness.snapshot());
        expect(snapshot.frames.every((frame) => frame.decoded === 0)).toBe(true);
        await page.evaluate(() => window.sfuHarness.stop());
      }
    } finally { await alice.close(); await bob.close(); }
  });
});

