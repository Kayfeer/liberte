import { test, expect, type Page, type TestInfo } from "@playwright/test";
import { gzipSync } from "node:zlib";
import { createHash } from "node:crypto";

declare global { interface Window { uiUnexpectedCommands: string[] } }

async function capture(page: Page, info: TestInfo, name: string) {
  const buffer = await page.screenshot({ path: info.outputPath(name + ".png"), animations: "disabled" });
  await info.attach(name, { body: buffer, contentType: "image/png" });
  // CI logs make the same screenshot reviewable when artifact downloads are unavailable.
  console.log("CODEX_UI_SCREENSHOT " + JSON.stringify({
    name: name + ".png", sha256: createHash("sha256").update(buffer).digest("hex"),
    gzip_base64: gzipSync(buffer).toString("base64"),
  }));
}

for (const width of [1280, 900]) {
  test("application navigation and Tailwind styles at " + width + "px (mock IPC)", async ({ page }, info) => {
    await page.setViewportSize({ width, height: 800 });
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto("/e2e/app-ui.html");
    await page.getByRole("button", { name: "Général", exact: true }).click();
    await expect(page.getByText("Bienvenue dans cet aperçu", { exact: false })).toBeVisible();
    await expect(page.getByRole("button", { name: "Rejoindre l'appel", exact: true })).toBeVisible();
    await expect(page.getByLabel("Transport de l'appel")).toBeVisible();
    const body = await page.locator("body").evaluate((element) => ({
      background: getComputedStyle(element).backgroundColor,
      color: getComputedStyle(element).color,
      width: element.scrollWidth,
    }));
    expect(body.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(body.color).not.toBe(body.background);
    expect(body.width).toBeLessThanOrEqual(width);
    await capture(page, info, "home-" + width);

    await page.getByRole("button", { name: "Paramètres", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Paramètres", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Identité", exact: true })).toBeVisible();
    const panel = page.locator(".panel").first();
    await expect(panel).toBeVisible();
    expect(await panel.evaluate((element) => getComputedStyle(element).borderRadius)).not.toBe("0px");
    await capture(page, info, "settings-" + width);

    const address = page.getByLabel("Adresse d'un pair ou d'un relais");
    await address.scrollIntoViewIfNeeded();
    await expect(address).toBeVisible();
    await expect(page.getByRole("button", { name: "Se connecter", exact: true })).toBeDisabled();
    expect(await address.evaluate((element) => getComputedStyle(element).backgroundColor)).not.toBe("rgba(0, 0, 0, 0)");
    await page.getByRole("button", { name: "Paramètres", exact: true }).click();
    await expect(page.getByRole("button", { name: "Rejoindre l'appel", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.uiUnexpectedCommands)).toEqual([]);
    expect(errors).toEqual([]);
  });
}

