import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { invokeTauri } from "@/shared/api/tauri";

/**
 * Workforce SSO (Keycloak OIDC) client for the Griddle desktop app.
 *
 * Flow (PKCE Authorization Code, system browser):
 *  1. `startOidcLogin()` fetches `/auth/oidc/start` from the company relay,
 *     which returns a fully-built Keycloak authorization URL plus the PKCE
 *     verifier + state it generated for us.
 *  2. The authorization URL opens in the system browser; the employee signs
 *     in with their work identity (Google broker behind Keycloak).
 *  3. Keycloak redirects to `griddle://auth/callback?code=…&state=…`, which
 *     macOS routes back into this app (deep-link scheme registered in
 *     tauri.conf.json and parsed in src-tauri/src/deep_link.rs).
 *  4. The Rust layer emits `oidc-auth-callback`; this module validates the
 *     `state` echo, then POSTs `/auth/oidc/complete` with the code + verifier.
 *  5. The relay validates the Keycloak token, mints/recovers the employee's
 *     Nostr keypair, admits them, and returns `{ pubkey, private_key }`.
 *  6. The private key is imported exactly like a manual key paste — it lands
 *     in the OS keychain through the same `import_identity` path.
 */

/** HTTP origin of the company relay. REST calls (OIDC) use plain HTTPS. */
export const GRIDDLE_RELAY_HTTP_ORIGIN = "https://griddle.patty.io";

type OidcStartResponse = {
  authorization_url: string;
  state: string;
  code_verifier: string;
};

export type OidcCompleteResponse = {
  pubkey: string;
  private_key: string;
  email: string;
  name: string | null;
  /** Workspace ws(s):// URL the client should join after sign-in. */
  relay_url: string;
};

export type OidcCallback = { code: string; state: string };

/** Pending flow state, held only in memory. */
let pending: {
  verifier: string;
  state: string;
  resolve: (keys: OidcCompleteResponse) => void;
  reject: (error: Error) => void;
  unlisten: UnlistenFn;
  timeout: ReturnType<typeof setTimeout>;
} | null = null;

/**
 * Begin workforce sign-in. Opens the system browser at the Keycloak
 * authorization URL and resolves with the employee's Nostr keypair once the
 * OIDC round-trip completes. Rejects on state mismatch, relay errors, or a
 * 3-minute timeout.
 */
export function startOidcLogin(
  relayOrigin = GRIDDLE_RELAY_HTTP_ORIGIN,
): Promise<OidcCompleteResponse> {
  if (pending) {
    return Promise.reject(new Error("An SSO login is already in progress"));
  }

  return new Promise<OidcCompleteResponse>((resolve, reject) => {
    void (async () => {
      // 1. Ask the relay for the authorization URL + PKCE verifier.
      let start: OidcStartResponse;
      try {
        const resp = await fetch(`${relayOrigin}/auth/oidc/start`, {
          method: "GET",
          headers: { Accept: "application/json" },
        });
        if (!resp.ok) {
          throw new Error(`relay /auth/oidc/start returned ${resp.status}`);
        }
        start = (await resp.json()) as OidcStartResponse;
      } catch (e) {
        reject(new Error(`Could not reach Griddle SSO: ${String(e)}`));
        return;
      }

      // 2. Listen for the deep-link callback before opening the browser.
      let unlisten: UnlistenFn | undefined;
      try {
        unlisten = await listen<OidcCallback>("oidc-auth-callback", (event) => {
          const cb = event.payload;
          if (!pending) return;
          if (cb.state !== pending.state) {
            pending.reject(
              new Error("SSO state mismatch — possible CSRF, aborting"),
            );
            return;
          }
          const { verifier } = pending;
          void (async () => {
            try {
              // 3–4. Exchange the code at the relay.
              const completeResp = await fetch(
                `${relayOrigin}/auth/oidc/complete`,
                {
                  method: "POST",
                  headers: { "Content-Type": "application/json" },
                  body: JSON.stringify({
                    code: cb.code,
                    code_verifier: verifier,
                  }),
                },
              );
              if (!completeResp.ok) {
                const body = await completeResp.text().catch(() => "");
                throw new Error(
                  `SSO failed (${completeResp.status}): ${body.slice(0, 200)}`,
                );
              }
              const keys = (await completeResp.json()) as OidcCompleteResponse;
              cleanupPending();
              resolve(keys);
            } catch (e) {
              cleanupPending();
              reject(new Error(`SSO completion failed: ${String(e)}`));
            }
          })();
        });
      } catch (e) {
        reject(new Error(`deep-link listener failed: ${String(e)}`));
        return;
      }

      // 3-minute cap so a stale pending flow can't wedge future attempts.
      const timeout = setTimeout(
        () => {
          if (!pending) return;
          cleanupPending();
          reject(
            new Error("SSO login timed out (browser did not redirect back)"),
          );
        },
        3 * 60 * 1000,
      );

      pending = {
        verifier: start.code_verifier,
        state: start.state,
        resolve,
        reject,
        unlisten,
        timeout,
      };

      // 3. Hand off to the system browser.
      try {
        await openUrl(start.authorization_url);
      } catch (e) {
        cleanupPending();
        reject(new Error(`Could not open browser: ${String(e)}`));
      }
    })();
  });
}

/** Cancel an in-flight SSO flow (e.g. the user navigated away). */
export function cancelOidcLogin(): void {
  if (!pending) return;
  const { reject } = pending;
  cleanupPending();
  reject(new Error("SSO login cancelled"));
}

function cleanupPending(): void {
  if (!pending) return;
  clearTimeout(pending.timeout);
  pending.unlisten();
  pending = null;
}

/**
 * Resolve the workspace email for the signed-in identity via the relay
 * (`oidc_whoami` Tauri command → GET /auth/oidc/whoami, NIP-98 authed).
 * Falls back to the locally cached SSO email. Returns "" when unknown.
 *
 * The result is cached in localStorage so the username survives restarts
 * without a network round-trip on the profile screen.
 */
export async function resolveWorkspaceEmail(): Promise<string> {
  const debug = (msg: string, data?: unknown) =>
    console.info(`[griddle-sso] ${msg}`, data ?? "");
  debug("resolveWorkspaceEmail: start");
  try {
    const email = await invokeTauri<string>("oidc_whoami");
    debug("whoami returned", email);
    if (email && email.includes("@")) {
      try {
        localStorage.setItem("griddle.ssoEmail", email);
      } catch {
        /* cache write best-effort */
      }
      debug("cached email");
      return email;
    }
    debug("whoami returned non-email value; falling back to cache");
  } catch (e) {
    debug("whoami invoke FAILED", String(e));
  }
  try {
    const cached = localStorage.getItem("griddle.ssoEmail") ?? "";
    debug("cache fallback", cached);
    return cached;
  } catch {
    return "";
  }
}
