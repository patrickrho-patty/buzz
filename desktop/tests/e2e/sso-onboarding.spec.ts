/**
 * Workforce SSO onboarding UI contract:
 *  - Username prefilled from the workspace email prefix, disabled/read-only
 *  - "Managed by Patty" helper note present
 *  - NO step-dot chrome (the 7-dot track must not render)
 *  - NO back button
 *  - After Continue (profile save), the flow goes to entering directly —
 *    no agent starter-team intro.
 */
import { expect, test } from "@playwright/test";

import { installMockBridge } from "../helpers/bridge";

const SSO_EMAIL = "e2e-test@patty.io";

test.beforeEach(async ({ page }) => {
  // Cold-cache mode: no localStorage seed — the prefill must come from the
  // whoami mock alone (the real cold-start path).

  await page.addInitScript((email) => {
    (window as unknown as Record<string, string>).__CREW_E2E_WHOAMI_EMAIL__ =
      email;
  }, SSO_EMAIL);
  await installMockBridge(
    page,
    { profileHasEvent: false },
    { skipCommunitySeed: true },
  );

  // Land directly in community onboarding with a first-community
  // transaction (the SSO join class — no deep-link acknowledge machinery).
  await page.addInitScript(() => {
    window.localStorage.setItem(
      "crew-community-onboarding-transaction.v1",
      JSON.stringify({
        id: crypto.randomUUID(),
        source: "first-community",
        firstCommunityPage: undefined,
        stage: "profile",
        relayUrl: "ws://localhost:3000",
        communityName: "Patty",
        inviteCode: undefined,
        token: undefined,
        reposDir: undefined,
        policyReceipt: undefined,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        acknowledged: true,
      }),
    );
    // Machine onboarding completion is keyed per-pubkey ("prefix:pubkey").
    // deadbeef×8 is the mock bridge identity.
    window.localStorage.setItem(
      "crew-machine-onboarding-complete.v2:" + "deadbeef".repeat(8),
      "true",
    );
  });

  await page.goto("/");
  await page.getByTestId("community-onboarding-flow").waitFor();
});

test("username is prefilled from email prefix and locked", async ({ page }) => {
  const input = page.getByTestId("community-profile-name-key");
  await expect(input).toHaveValue("e2e-test");
  await expect(input).toBeDisabled();
  await expect(
    page.getByText("Managed by Patty — your username follows"),
  ).toBeVisible();
});

test("whoami invoke carries the onboarding transaction relay URL", async ({
  page,
}) => {
  // Regression pin: the real oidc_whoami command used to resolve its base
  // URL from build defaults, so a cold-cache fresh onboarding queried
  // whatever listened on localhost:3000 instead of the joining workspace.
  const input = page.getByTestId("community-profile-name-key");
  await expect(input).toHaveValue("e2e-test");

  const whoamiPayloads = await page.evaluate(() =>
    (window.__CREW_E2E_COMMAND_LOG__ ?? [])
      .filter((entry) => entry.command === "oidc_whoami")
      .map((entry) => entry.payload),
  );
  expect(whoamiPayloads.length).toBeGreaterThan(0);
  expect(whoamiPayloads[0]).toMatchObject({
    relayUrl: "ws://localhost:3000",
  });
});

test("no step-dot chrome during SSO join", async ({ page }) => {
  await expect(page.getByTestId("onboarding-step-dots")).toHaveCount(0);
});

test("no back button during SSO join", async ({ page }) => {
  await expect(page.getByTestId("community-profile-back")).toHaveCount(0);
});

test("continue skips the agent starter-team intro", async ({ page }) => {
  await page.getByTestId("community-profile-next").click();
  // The agent starter-team content must never render during SSO finalize…
  await expect(page.getByText("Meet your starter team")).toHaveCount(0);
  await expect(page.getByTestId("starter-persona-fizz")).toHaveCount(0);
  // …and the flow finalizes into the app (entering curtain or app shell).
  await expect(
    page
      .getByTestId("onboarding-entering-curtain")
      .or(page.getByTestId("community-app")),
  ).toBeVisible({ timeout: 15_000 });
});
