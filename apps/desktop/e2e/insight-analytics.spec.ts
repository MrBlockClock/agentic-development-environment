import { expect, test } from "@playwright/test";
import { installTauriStub } from "./fixtures/tauriStub";

/**
 * Analytics is Desktop-only (the usage ledger is read over IPC), so this spec
 * stubs the Tauri bridge and asserts the money math the surface promises:
 * committed actuals and open reserves stay separate, and a settled turn priced
 * at $0 is reported rather than averaged away.
 */
test.describe("Insight · Analytics", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriStub(page);
    await page.addInitScript(() => {
      window.localStorage.setItem("ade_insight_section", "Analytics");
    });
  });

  async function openAnalytics(page: import("@playwright/test").Page) {
    await page.goto("/");
    await page
      .getByTestId("ade-sidebar")
      .getByRole("button", { name: "Insight" })
      .click();
    const tabs = page.getByRole("tablist", { name: "Insight sections" });
    await tabs.getByRole("tab", { name: "Analytics" }).click();
    return page.getByTestId("ade-analytics");
  }

  test("separates committed actuals from open reserves", async ({ page }) => {
    const analytics = await openAnalytics(page);

    // 7 settled rows in the default 7-day window total $2.30.
    await expect(
      analytics.getByTestId("ade-metric-committed-spend"),
    ).toContainText("$2.30");
    await expect(analytics.getByTestId("ade-metric-committed-spend")).toContainText(
      "7 settled turns",
    );

    // The one unsettled row stays an estimate and is never folded into spend.
    const reserve = analytics.getByTestId("ade-metric-open-reserve");
    await expect(reserve).toContainText("$0.25");
    await expect(reserve).toContainText("est");

    await expect(analytics.getByTestId("ade-metric-remaining-today")).toContainText(
      "$8.97",
    );
  });

  test("reports reserve drift and $0-priced turns", async ({ page }) => {
    const analytics = await openAnalytics(page);
    const accuracy = analytics.getByTestId("ade-analytics-reserve-accuracy");

    // reserved $3.08 against actual $2.30 — over-reserving, not under.
    await expect(accuracy).toContainText("+$0.78");
    await expect(accuracy).toContainText("reserved $3.08 → actual $2.30");
    await expect(accuracy).toContainText("1 priced turn reported $0");
  });

  test("attributes spend to the model that earned it", async ({ page }) => {
    const analytics = await openAnalytics(page);
    const byModel = analytics.getByTestId("ade-analytics-by-model");

    // Ranked by committed spend: sonnet $0.95 edges out opus $0.94.
    const rows = byModel.getByTestId("ade-stat-row");
    await expect(rows.first()).toContainText("claude-sonnet-4");
    await expect(rows.first()).toContainText("$0.95");

    // A model that settled at $0 still has to appear, at 0%.
    await expect(byModel).toContainText("gemini-2.5-pro");
  });

  test("window switch re-scopes the totals", async ({ page }) => {
    const analytics = await openAnalytics(page);
    const committed = analytics.getByTestId("ade-metric-committed-spend");
    await expect(committed).toContainText("$2.30");

    // A $1.05 turn from 12 days ago only belongs to the wider windows.
    await analytics.getByRole("button", { name: "30 days" }).click();
    await expect(committed).toContainText("$3.35");

    await analytics.getByRole("button", { name: "Today" }).click();
    await expect(committed).toContainText("$0.78");
  });
});
