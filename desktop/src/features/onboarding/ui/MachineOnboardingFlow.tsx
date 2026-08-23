import * as React from "react";
import type { QueryClient } from "@tanstack/react-query";
import { motion, useReducedMotion } from "motion/react";

import {
  getIdentity,
  importIdentity,
  persistCurrentIdentity,
} from "@/shared/api/tauriIdentity";
import type { IdentityStorage } from "@/shared/api/types";
import { Button } from "@/shared/ui/button";
import { startOidcLogin } from "@/shared/auth/oidcClient";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/shared/ui/dialog";
import { StartupWindowDragRegion } from "@/shared/ui/StartupWindowDragRegion";
import { BackupStep } from "./BackupStep";
import { DownloadKeyStep } from "./DownloadKeyStep";
import {
  resetEncryptedBackupSession,
  useEncryptedBackupSession,
} from "./EncryptedBackupCreator";
import { IdentityRecoveryPairing } from "./IdentityRecoveryPairing";
import { LandingBees } from "./LandingBees";
import {
  NostrKeyImportForm,
  type NostrKeyImportStage,
} from "./NostrKeyImportForm";
import {
  ONBOARDING_INK_ICON_CLASS,
  ONBOARDING_LANDING_CTA_CLASS,
  ONBOARDING_SECONDARY_CTA_CLASS,
  OnboardingChrome,
} from "./OnboardingChrome";
import { OnboardingFooterProvider } from "./OnboardingFooter";
import {
  type OnboardingTransitionDirection,
  OnboardingSlideTransition,
} from "./OnboardingSlideTransition";

export type MachineOnboardingPage =
  | "identity"
  | "key-import"
  | "backup"
  | "setup"
  | "config";

type BackupSubview = "created" | "options" | "password";

/** A pending navigation the parent should execute after RouterProvider mounts. */
export type PostOnboardingNavigation = {
  to: string;
  search?: Record<string, string>;
};

export function MachineOnboardingFlow({
  complete,
  continueWithIdentity,
  continueWithRecoveredIdentity,
  identityLost,
  initialPage,
  queryClient,
  onSsoWorkspace,
}: {
  complete: (pubkey?: string) => void;
  continueWithIdentity: (pubkey: string) => void;
  continueWithRecoveredIdentity: (pubkey: string) => void;
  identityLost: boolean;
  initialPage?: MachineOnboardingPage;
  queryClient: QueryClient;
  /**
   * Workforce SSO handoff: called with the workspace relay URL after a
   * successful Keycloak login. The default post-identity pages (harness
   * setup, provider config, community picker) are skipped — the user goes
   * straight into their company workspace.
   */
  onSsoWorkspace?: (result: {
    pubkey: string;
    relayUrl: string;
    email: string;
  }) => void;
}) {
  const [page, setPage] = React.useState<MachineOnboardingPage>(
    identityLost ? "key-import" : (initialPage ?? "identity"),
  );
  const [transitionDirection, setTransitionDirection] =
    React.useState<OnboardingTransitionDirection>("forward");
  const [error, setError] = React.useState<string | null>(null);
  const [showManualKeyOptions, setShowManualKeyOptions] = React.useState(false);
  const [isPending, setIsPending] = React.useState(false);
  const [keyImportStage, setKeyImportStage] =
    React.useState<NostrKeyImportStage>("key-entry");
  const [isKeyImporting, setIsKeyImporting] = React.useState(false);
  const [keyImportFormKey, setKeyImportFormKey] = React.useState(0);
  const [keyImportDialog, setKeyImportDialog] = React.useState<
    "backup" | "phone" | null
  >(null);
  const [phoneRecoveryStep, setPhoneRecoveryStep] = React.useState("loading");
  const [selectedPubkey, setSelectedPubkey] = React.useState<string | null>(
    null,
  );
  const [identityStorage, setIdentityStorage] = React.useState<
    IdentityStorage | undefined
  >();
  const [backupSubview, setBackupSubview] =
    React.useState<BackupSubview>("created");
  const [backupDirection, setBackupDirection] = React.useState<
    "forward" | "backward"
  >("forward");
  const [returningFromSecurity, setReturningFromSecurity] =
    React.useState(false);
  // Owned here so switching between the yellow onboarding view and the dark
  // security subview keeps the created backup, password, and test progress.
  const backupSession = useEncryptedBackupSession();
  const reduceMotion = useReducedMotion() ?? false;
  const isSecuritySubview = page === "backup" && backupSubview !== "created";
  const loadFreshIdentity = React.useCallback(async () => {
    setIsPending(true);
    setError(null);
    try {
      const identity = await getIdentity();
      queryClient.setQueryData(["identity"], identity);
      setSelectedPubkey(identity.pubkey);
      setIdentityStorage(identity.storage);
      setBackupDirection("forward");
      setTransitionDirection("forward");
      setReturningFromSecurity(false);
      setBackupSubview("created");
      setPage("backup");
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : "Failed to load identity",
      );
    } finally {
      setIsPending(false);
    }
  }, [queryClient]);

  const loadRecoveredIdentity = React.useCallback(async () => {
    setIsPending(true);
    setError(null);
    try {
      const identity = await getIdentity();
      continueWithRecoveredIdentity(identity.pubkey);
      queryClient.setQueryData(["identity"], identity);
      setSelectedPubkey(identity.pubkey);
      setIdentityStorage(identity.storage);
      // Workforce build: harness/provider setup is done later in Settings →
      // Agents; onboarding completes as soon as the identity is recovered.
      complete(identity.pubkey);
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : "Failed to load identity",
      );
    } finally {
      setIsPending(false);
    }
  }, [continueWithRecoveredIdentity, queryClient]);

  const replaceLostIdentity = React.useCallback(async () => {
    const confirmed = window.confirm(
      "This will create a new identity and abandon your previous key. This cannot be undone. Continue?",
    );
    if (!confirmed) return;

    setIsPending(true);
    setError(null);
    try {
      const identity = await persistCurrentIdentity();
      queryClient.setQueryData(["identity"], identity);
      setSelectedPubkey(identity.pubkey);
      setIdentityStorage(identity.storage);
      setBackupDirection("forward");
      setTransitionDirection("forward");
      setReturningFromSecurity(false);
      setBackupSubview("created");
      setPage("backup");
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : "Failed to save identity",
      );
    } finally {
      setIsPending(false);
    }
  }, [queryClient]);

  const importExistingIdentity = React.useCallback(
    async (nsec: string, password?: string) => {
      const identity = await importIdentity(nsec, password);
      continueWithIdentity(identity.pubkey);
      queryClient.setQueryData(["identity"], identity);
      setSelectedPubkey(identity.pubkey);
      // Workforce build: harness/provider setup is done later in Settings →
      // Agents; onboarding completes as soon as the key is imported.
      complete(identity.pubkey);
    },
    [complete, continueWithIdentity, queryClient],
  );

  // Workforce SSO: open the system browser at Keycloak; when the OIDC
  // round-trip completes, the relay hands back the employee's Nostr keypair,
  // which flows through the exact same import path as a manual paste.
  const signInWithPatty = React.useCallback(async () => {
    setIsPending(true);
    setError(null);
    try {
      const keys = await startOidcLogin();
      console.info("[griddle-sso] SSO login result", {
        pubkey: keys.pubkey?.slice(0, 12),
        email: keys.email,
        relayUrl: keys.relay_url,
      });
      const identity = await importIdentity(keys.private_key);
      continueWithIdentity(identity.pubkey);
      queryClient.setQueryData(["identity"], identity);
      setSelectedPubkey(identity.pubkey);
      if (onSsoWorkspace) {
        // Workforce path: skip harness/provider/community-picker pages and
        // hand the caller the workspace URL (it completes onboarding and
        // starts community connection for the company relay).
        onSsoWorkspace({
          pubkey: identity.pubkey,
          relayUrl: keys.relay_url,
          email: keys.email,
        });
      } else {
        complete(identity.pubkey);
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "SSO sign-in failed");
    } finally {
      setIsPending(false);
    }
  }, [continueWithIdentity, onSsoWorkspace, queryClient]);

  const backFromKeyImport = React.useCallback(() => {
    if (keyImportStage === "backup-password") {
      setKeyImportFormKey((current) => current + 1);
      setKeyImportStage("key-entry");
      return;
    }
    setTransitionDirection("backward");
    setPage("identity");
  }, [keyImportStage]);

  const returnToCreatedKey = React.useCallback(() => {
    setBackupDirection("backward");
    setReturningFromSecurity(true);
    setBackupSubview("created");
  }, []);

  const backFromPasswordBackup = React.useCallback(() => {
    resetEncryptedBackupSession(backupSession);
    setBackupDirection("backward");
    setReturningFromSecurity(false);
    setBackupSubview("options");
  }, [backupSession]);

  const chromeBackAction =
    page === "key-import" &&
    (!identityLost || keyImportStage === "backup-password")
      ? { disabled: isKeyImporting, onClick: backFromKeyImport }
      : page === "backup" && backupSubview !== "created"
        ? {
            label: "Return to onboarding",
            onClick: returnToCreatedKey,
            testId: "backup-return-to-onboarding",
          }
        : page === "backup"
          ? {
              onClick: () => {
                setTransitionDirection("backward");
                setPage("identity");
              },
            }
          : undefined;

  return (
    <div
      className={`buzz-onboarding-neutral-theme buzz-startup-shell flex max-h-dvh items-start justify-center overflow-x-hidden overflow-y-auto px-4 text-foreground ${
        isSecuritySubview ? "buzz-onboarding-security-theme" : ""
      } ${
        page === "identity"
          ? "buzz-onboarding-welcome py-8"
          : "pb-28 pt-[106px]"
      }`}
      data-testid="machine-onboarding-gate"
    >
      <StartupWindowDragRegion />
      {page === "identity" ? <LandingBees /> : null}
      {page !== "identity" && !isSecuritySubview ? (
        <OnboardingChrome
          current={page === "config" ? 4 : page === "setup" ? 3 : 2}
        />
      ) : null}
      <OnboardingFooterProvider backAction={chromeBackAction}>
        <div
          className={`relative flex w-full max-w-[1040px] flex-col items-center text-center ${
            page === "identity" ? "my-auto" : "buzz-onboarding-step-frame"
          }`}
        >
          {page === "identity" ? (
            <OnboardingSlideTransition
              className="flex w-full max-w-[720px] flex-col items-center text-center"
              direction={transitionDirection}
              transitionKey={`machine-identity-${transitionDirection}`}
            >
              <img
                alt="Griddle"
                className="w-full max-w-[600px]"
                src="/landing/griddle-wordmark.png"
              />
              <p className="mt-2 max-w-[560px] text-center text-2xl font-normal leading-none text-foreground">
                Your people, your agents, your projects —<br />
                all in one place.
              </p>
              {error ? (
                <p className="mt-4 text-sm text-destructive">{error}</p>
              ) : null}
              <div className="mt-10 flex flex-col items-center gap-3">
                <Button
                  className={ONBOARDING_LANDING_CTA_CLASS}
                  disabled={isPending}
                  onClick={() => void signInWithPatty()}
                  type="button"
                >
                  {isPending
                    ? "Waiting for browser sign-in…"
                    : "Sign in with Patty"}
                </Button>
                {/* Manual key options: collapse behind a quiet footnote toggle —
                    workforce SSO is the primary path; manual import stays
                    reachable for the relay owner and edge cases. */}
                {showManualKeyOptions ? (
                  <>
                    <Button
                      className={`${ONBOARDING_SECONDARY_CTA_CLASS} px-5`}
                      disabled={isPending}
                      onClick={() => void loadFreshIdentity()}
                      type="button"
                    >
                      {selectedPubkey
                        ? "Continue setup"
                        : "Create a new identity key"}
                    </Button>
                    <Button
                      className={`${ONBOARDING_SECONDARY_CTA_CLASS} px-5`}
                      disabled={isPending}
                      onClick={() => {
                        setKeyImportDialog(null);
                        setKeyImportStage("key-entry");
                        setTransitionDirection("forward");
                        setPage("key-import");
                      }}
                      type="button"
                      variant="ghost"
                    >
                      {selectedPubkey
                        ? "Use a different key instead"
                        : "Use an existing key"}
                    </Button>
                  </>
                ) : (
                  <button
                    className="text-xs text-foreground/50 underline-offset-4 hover:underline"
                    data-testid="show-manual-key-options"
                    onClick={() => setShowManualKeyOptions(true)}
                    type="button"
                  >
                    Advanced: use a key manually
                  </button>
                )}
              </div>
            </OnboardingSlideTransition>
          ) : page === "key-import" ? (
            <OnboardingSlideTransition
              className="flex min-h-[calc(100dvh-13.25rem)] w-full max-w-[837px] flex-col items-center text-center"
              direction={transitionDirection}
              transitionKey={`machine-key-import-${transitionDirection}`}
            >
              <motion.div
                animate={{ opacity: 1, y: 0 }}
                className="relative z-10 shrink-0"
                initial={reduceMotion ? false : { opacity: 0, y: 10 }}
                key={keyImportStage}
                transition={{
                  duration: reduceMotion ? 0 : 0.3,
                  ease: "easeOut",
                }}
              >
                <h1 className="text-title font-normal text-foreground">
                  {keyImportStage === "backup-password"
                    ? "Unlock your account"
                    : "Enter your private key"}
                </h1>
                <div className="mt-5 max-w-[440px] text-sm leading-6 text-foreground/80">
                  {keyImportStage === "backup-password" ? (
                    "Enter your backup password to restore your identity."
                  ) : (
                    <p>
                      Paste your private key to sign in to Buzz. You can also
                      use a{" "}
                      <button
                        className="rounded-sm font-medium underline decoration-foreground/40 underline-offset-4 transition-colors hover:decoration-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:opacity-60"
                        data-testid="nostr-import-file-button"
                        disabled={isPending}
                        onClick={() => setKeyImportDialog("backup")}
                        type="button"
                      >
                        backup file
                      </button>
                      , or{" "}
                      <button
                        className="rounded-sm font-medium underline decoration-foreground/40 underline-offset-4 transition-colors hover:decoration-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:opacity-60"
                        data-testid="nostr-import-phone-link"
                        disabled={isPending}
                        onClick={() => setKeyImportDialog("phone")}
                        type="button"
                      >
                        recover from your phone
                      </button>
                      .
                    </p>
                  )}
                </div>
              </motion.div>
              <div className="buzz-onboarding-key-import-position w-full">
                <div className="flex flex-col items-center">
                  <NostrKeyImportForm
                    key={keyImportFormKey}
                    onBack={backFromKeyImport}
                    onImport={importExistingIdentity}
                    onImportingChange={setIsKeyImporting}
                    onStageChange={setKeyImportStage}
                    showBack={false}
                    showPasswordStageBack={false}
                    variant="spotlight"
                  />
                  {identityLost && keyImportStage === "key-entry" ? (
                    <Button
                      className={`${ONBOARDING_SECONDARY_CTA_CLASS} mt-2 px-5`}
                      disabled={isPending || isKeyImporting}
                      onClick={() => void replaceLostIdentity()}
                      type="button"
                      variant="ghost"
                    >
                      Start new identity
                    </Button>
                  ) : null}
                </div>
              </div>
              <Dialog
                onOpenChange={(open) => {
                  if (!open) setKeyImportDialog(null);
                }}
                open={keyImportDialog === "backup"}
              >
                <DialogContent
                  className="buzz-onboarding-neutral-theme max-w-[47.5rem] -translate-y-5"
                  closeButtonClassName={ONBOARDING_INK_ICON_CLASS}
                  data-system-color-scheme="light"
                  data-testid="backup-recovery-dialog"
                  surface="textured"
                >
                  <div className="mx-auto w-full max-w-[35rem] pb-6 pt-10 text-center max-sm:pb-4 max-sm:pt-6">
                    <DialogTitle className="text-balance px-8 text-3xl font-normal text-foreground">
                      Restore from a backup file
                    </DialogTitle>
                    <DialogDescription className="mx-auto mt-4 max-w-[28rem] text-sm leading-6 text-foreground/80">
                      Choose the encrypted backup file you saved from Buzz.
                    </DialogDescription>
                    <NostrKeyImportForm
                      footerMode="inline"
                      mode="backup"
                      onBack={() => setKeyImportDialog(null)}
                      onImport={importExistingIdentity}
                      showBack={false}
                      variant="spotlight"
                    />
                  </div>
                </DialogContent>
              </Dialog>
              <Dialog
                onOpenChange={(open) => {
                  if (!open) setKeyImportDialog(null);
                }}
                open={keyImportDialog === "phone"}
              >
                <DialogContent
                  className="buzz-onboarding-neutral-theme max-h-[calc(100dvh-2rem)] max-w-[47.5rem] -translate-y-5 overflow-y-auto"
                  closeButtonClassName={ONBOARDING_INK_ICON_CLASS}
                  data-system-color-scheme="light"
                  data-testid="phone-recovery-dialog"
                  surface="textured"
                >
                  <div className="mx-auto flex w-full max-w-[35rem] flex-col items-center pb-6 pt-8 text-center max-sm:pb-4 max-sm:pt-4">
                    <DialogTitle className="text-balance px-8 text-3xl font-normal text-foreground">
                      {identityLost
                        ? "Recover from your phone"
                        : "Use your Buzz identity"}
                    </DialogTitle>
                    <DialogDescription className="mt-4 text-sm leading-6 text-foreground/80">
                      {phoneRecoveryStep === "loading" ||
                      phoneRecoveryStep === "qr"
                        ? "Scan this code with a signed-in Buzz phone."
                        : "Confirm the code before sharing your identity."}
                    </DialogDescription>
                    <div className="mt-5">
                      <IdentityRecoveryPairing
                        onRecovered={loadRecoveredIdentity}
                        onStepChange={setPhoneRecoveryStep}
                      />
                    </div>
                  </div>
                </DialogContent>
              </Dialog>
            </OnboardingSlideTransition>
          ) : page === "backup" ? (
            backupSubview === "password" ? (
              <DownloadKeyStep
                direction={backupDirection}
                onBack={backFromPasswordBackup}
                session={backupSession}
              />
            ) : (
              <BackupStep
                direction={backupDirection}
                identityStorage={identityStorage}
                onNext={() => {
                  // Workforce build: after key backup, finish onboarding
                  // (harness/provider setup lives in Settings → Agents).
                  complete(selectedPubkey ?? undefined);
                }}
                onOpenPasswordBackup={() => {
                  resetEncryptedBackupSession(backupSession);
                  setBackupDirection("forward");
                  setReturningFromSecurity(false);
                  setBackupSubview("password");
                }}
                onShowOptions={() => {
                  setBackupDirection("forward");
                  setReturningFromSecurity(false);
                  setBackupSubview("options");
                }}
                optionsExpanded={backupSubview === "options"}
                returningFromSecurity={returningFromSecurity}
              />
            )
          ) : page === "setup" || page === "config" ? (
            // Workforce build: the agent-harness (setup) and model-provider
            // (config) pages are disabled — agent configuration lives in
            // Settings → Agents. Kept as a defensive branch: if any stale
            // state lands here, complete onboarding instead of showing the
            // wizard pages.
            <div className="flex min-h-[40vh] items-center justify-center">
              <Button
                className={ONBOARDING_LANDING_CTA_CLASS}
                onClick={() => complete(selectedPubkey ?? undefined)}
                type="button"
              >
                Continue
              </Button>
            </div>
          ) : null}
        </div>
      </OnboardingFooterProvider>
    </div>
  );
}
