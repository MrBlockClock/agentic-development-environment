import { expect, test } from "@playwright/test";

/**
 * Self-improve smoke: catch sidebar type / IA regressions in browser preview.
 * Desktop Tauri paths remain covered by dogfood + verify ladder (G0–G5).
 */
test.describe("ADE sidebar IA", () => {
  test("sidebar exposes Home, workplace, sessions, Setup fold", async ({
    page,
  }) => {
    await page.goto("/");
    const sidebar = page.getByTestId("ade-sidebar");
    await expect(sidebar).toBeVisible();

    await expect(sidebar.getByRole("button", { name: "Home" })).toBeVisible();
    await expect(page.getByTestId("ade-rail-context")).toBeVisible();
    await expect(page.getByTestId("ade-session-list")).toBeVisible();
    await expect(page.getByTestId("ade-nav-fold-setup")).toBeVisible();
  });

  test("Home, Sessions, and Setup share the same type size", async ({
    page,
  }) => {
    await page.goto("/");
    const home = page.getByTestId("ade-nav-home").getByRole("button").first();
    const sessions = page.getByTestId("ade-sessions-label");
    const setup = page.getByTestId("ade-nav-fold-setup");
    await expect(home).toBeVisible();
    await expect(sessions).toBeVisible();
    await expect(setup).toBeVisible();

    const sizes = await Promise.all(
      [home, sessions, setup].map((locator) =>
        locator.evaluate((el) => Number.parseFloat(getComputedStyle(el).fontSize)),
      ),
    );
    const [homeSize, sessionsSize, setupSize] = sizes;
    expect(Math.abs(homeSize - sessionsSize)).toBeLessThanOrEqual(0.5);
    expect(Math.abs(homeSize - setupSize)).toBeLessThanOrEqual(0.5);
    expect(homeSize).toBeGreaterThanOrEqual(12);
    expect(homeSize).toBeLessThanOrEqual(15);
  });

  test("Insight is one destination with Trust and Analytics sub-tabs", async ({
    page,
  }) => {
    await page.goto("/");
    const sidebar = page.getByTestId("ade-sidebar");

    // The four looking-surfaces collapse into one rail row.
    await expect(sidebar.getByRole("button", { name: "Insight" })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "Atlas" })).toHaveCount(0);
    await expect(sidebar.getByRole("button", { name: "Plan Map" })).toHaveCount(0);

    await sidebar.getByRole("button", { name: "Insight" }).click();

    const tabs = page.getByRole("tablist", { name: "Insight sections" });
    await expect(tabs).toBeVisible();
    await expect(tabs.getByRole("tab", { name: "Trust" })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    // Analytics ships on Standard — it must not need Debug.
    const analytics = tabs.getByRole("tab", { name: "Analytics" });
    await expect(analytics).toBeVisible();
    await analytics.click();
    await expect(analytics).toHaveAttribute("aria-selected", "true");

    // Maps stay Debug density.
    await expect(tabs.getByRole("tab", { name: "Atlas" })).toHaveCount(0);
  });
});
