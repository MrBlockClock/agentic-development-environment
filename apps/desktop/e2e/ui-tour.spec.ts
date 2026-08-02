import { expect, test } from "@playwright/test";
import { installTauriStub } from "./fixtures/tauriStub";

/**
 * Visual/functional tour of current shell surfaces with a Tauri IPC stub.
 * Not a permanent CI gate — run: npx playwright test e2e/ui-tour.spec.ts
 */
test.describe("ADE UI tour (stubbed Desktop path)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriStub(page);
    await page.addInitScript(() => {
      window.localStorage.setItem("ade_surface_mode", "dev");
      window.localStorage.setItem("ade_dev_mode", "1");
    });
  });

  test("shell + setup + insight + integrations render", async ({ page }) => {
    await page.goto("/");
    const sidebar = page.getByTestId("ade-sidebar");
    await expect(sidebar).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "Home" })).toBeVisible();

    // Composer / agent shell on Home — empty canvas, not last-run transcript
    await expect(page.getByRole("tab", { name: "Agent" })).toBeVisible();
    await expect(page.getByTestId("ade-home-canvas")).toBeVisible();
    await expect(page.getByRole("heading", { name: "ADE", exact: true })).toBeVisible();
    await expect(page.getByTestId("ade-getting-started")).toBeVisible();
    await expect(page.getByText("Getting started")).toBeVisible();
    await page.screenshot({
      path: "e2e/artifacts/tour-home.png",
      fullPage: true,
    });

    // Keys (real view under stub)
    await sidebar.getByRole("button", { name: /Keys/ }).click();
    await expect(page.getByRole("heading", { name: "API keys" })).toBeVisible();
    await page.screenshot({
      path: "e2e/artifacts/tour-keys.png",
      fullPage: true,
    });

    // Integrations
    await sidebar.getByRole("button", { name: /Integrations/ }).click();
    await expect(page.getByTestId("ade-integrations")).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Integrations", exact: true }).nth(1),
    ).toBeVisible();
    await expect(page.getByRole("tab", { name: "Connectors" })).toBeVisible();
    await page.getByRole("tab", { name: /Host tools/ }).click();
    await expect(page.getByText("Tools this turn")).toBeVisible();
    await page.screenshot({
      path: "e2e/artifacts/tour-integrations.png",
      fullPage: true,
    });

    // Insight · Analytics (already covered by dedicated specs)
    await sidebar.getByRole("button", { name: "Insight" }).click();
    const tabs = page.getByRole("tablist", { name: "Insight sections" });
    await tabs.getByRole("tab", { name: "Analytics" }).click();
    await expect(page.getByTestId("ade-analytics")).toBeVisible();
    await page.screenshot({
      path: "e2e/artifacts/tour-analytics.png",
      fullPage: true,
    });

    // Settings
    await sidebar.getByRole("button", { name: "Settings" }).click();
    await expect(page.getByText("Defaults for Home")).toBeVisible();
    await page.screenshot({
      path: "e2e/artifacts/tour-settings.png",
      fullPage: true,
    });

    // Editor (Debug on)
    const editorNav = sidebar.getByRole("button", { name: /Editor/ });
    if ((await editorNav.count()) > 0) {
      await editorNav.click();
      await expect(page.getByText(/Monaco|Editor/)).toBeVisible();
      await page.screenshot({
        path: "e2e/artifacts/tour-editor.png",
        fullPage: true,
      });
    }
  });
});
