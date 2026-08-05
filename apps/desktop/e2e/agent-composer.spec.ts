import { expect, test } from "@playwright/test";
import { installTauriStub } from "./fixtures/tauriStub";

/**
 * Agent / composer smoke against Vite preview with a Tauri IPC stub.
 * No live LLM: assert Home mounts AgentView, composer gates Send correctly.
 */
test.describe("Home · Agent composer", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriStub(page);
  });

  test("Home shows ADE canvas and composer with Send gated on prompt", async ({
    page,
  }) => {
    await page.goto("/");

    await expect(page.getByTestId("ade-sidebar")).toBeVisible();
    await expect(page.getByTestId("ade-home-canvas")).toBeVisible();
    await expect(page.getByTestId("ade-home-canvas")).toContainText("ADE");

    const composer = page.getByTestId("ade-composer");
    await expect(composer).toBeVisible();
    await expect(composer).toHaveAttribute(
      "placeholder",
      /Ask ADE|Add to queue/,
    );

    const send = page.getByTestId("ade-send");
    await expect(send).toBeVisible();
    await expect(send).toBeDisabled();
    await expect(send).toHaveAttribute("title", "Send");

    await composer.fill("smoke: stubbed composer — do not call a model");
    await expect(send).toBeEnabled();

    // Clear → gate again (empty prompt, no attachments).
    await composer.fill("");
    await expect(send).toBeDisabled();
  });

  test("Suggest / Apply mode controls stay on the composer footer", async ({
    page,
  }) => {
    await page.goto("/");
    await expect(page.getByTestId("ade-composer")).toBeVisible();

    const mode = page.getByRole("button", { name: "Mode", exact: true });
    await expect(mode).toBeVisible();
    await expect(mode).toContainText(/Suggest|Apply/);

    const shell = page.getByRole("button", { name: "Shell scope", exact: true });
    await expect(shell).toBeVisible();
  });
});
