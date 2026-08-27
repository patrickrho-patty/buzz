#!/usr/bin/env python3
"""End-to-end workforce SSO test against the REAL stack.

Drives the complete employee flow with a headless browser:
  1. GET /auth/oidc/start  → authorization URL + PKCE verifier + state
  2. Headless Chromium opens the Keycloak authorization URL
  3. Fills username (e2e-test@patty.io) + password on the Keycloak login form
  4. Keycloak redirects to griddle://auth/callback?code=…&state=…
     (the browser cannot follow that scheme — we intercept the redirect URL,
      same as the OS would hand it to the desktop app)
  5. POST /auth/oidc/complete {code, code_verifier}
  6. Asserts: pubkey returned, email == e2e-test@patty.io,
     relay_members row added with added_by='oidc',
     Keycloak user now carries nostr_pubkey + nostr_privkey_encrypted
  7. Cleanup: removes the oidc membership row (Keycloak attrs kept to prove
     persistence; delete manually if undesired)

Exit code 0 = full pass. Any failure prints the failing step.
"""
import json
import subprocess
import sys
import urllib.parse
import urllib.request
import ssl

import base64
import hashlib
import secrets

RELAY = "https://griddle.patty.io"
KEYCLOAK_USER = "e2e-test@patty.io"
KEYCLOAK_PASS = "E2eTest!2026"

import certifi
ctx = ssl.create_default_context(cafile=certifi.where())


def req(method, url, data=None, headers=None):
    r = urllib.request.Request(url, data=data, headers=headers or {}, method=method)
    try:
        resp = urllib.request.urlopen(r, timeout=30, context=ctx)
        return resp.status, resp.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()


def step(n, msg):
    print(f"[{n}] {msg}")


def main():
    # 1. start
    step(1, "GET /auth/oidc/start")
    status, body = req("GET", f"{RELAY}/auth/oidc/start")
    assert status == 200, f"start failed {status}: {body[:200]}"
    start = json.loads(body)
    print(f"    state={start['state'][:12]}… verifier present")

    # 2-4. browser leg
    step(2, f"headless browser → Keycloak login as {KEYCLOAK_USER}")
    from playwright.sync_api import sync_playwright

    callback_query = None
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        # griddle:// is not navigable — catch the request that tries
        def on_request(request):
            nonlocal callback_query
            if request.url.startswith("griddle://"):
                callback_query = request.url

        page.on("request", on_request)
        page.goto(start["authorization_url"], wait_until="domcontentloaded")

        # Keycloak login form (username first, then password — or single page)
        page.wait_for_selector("#username", timeout=15000)
        page.fill("#username", KEYCLOAK_USER)
        page.fill("#password", KEYCLOAK_PASS)
        page.click("#kc-login")
        # The redirect to griddle:// fires; give it a moment
        page.wait_for_timeout(4000)
        browser.close()

    assert callback_query, "never saw griddle:// callback redirect"
    q = urllib.parse.urlparse(callback_query).query
    params = dict(urllib.parse.parse_qsl(q))
    code = params["code"]
    ret_state = params["state"]
    assert ret_state == start["state"], "state mismatch (CSRF guard tripped?)"
    print(f"    got code={code[:12]}… state matches")

    # 5. complete
    step(3, "POST /auth/oidc/complete")
    payload = json.dumps({"code": code, "code_verifier": start["code_verifier"]}).encode()
    status, body = req(
        "POST",
        f"{RELAY}/auth/oidc/complete",
        data=payload,
        headers={"Content-Type": "application/json"},
    )
    assert status == 200, f"complete failed {status}: {body[:300]}"
    result = json.loads(body)
    print(f"    pubkey={result['pubkey'][:16]}… email={result['email']} relay_url={result['relay_url']}")

    # 6a. email correctness
    assert result["email"] == KEYCLOAK_USER, f"email mismatch: {result['email']}"
    step(4, "✓ email is the workspace address")

    # 6b. Keycloak attribute persistence
    step(5, "verify Keycloak nostr attributes persisted")
    out = subprocess.run(
        ["kubectl", "-n", "griddle", "get", "secret", "griddle-oidc",
         "-o", "jsonpath={.data.CREW_OIDC_BRIDGE_CLIENT_SECRET}"],
        capture_output=True, text=True).stdout.strip()
    bridge_secret = base64.b64decode(out).decode()
    tok_body = urllib.parse.urlencode({
        "grant_type": "client_credentials",
        "client_id": "griddle-bridge",
        "client_secret": bridge_secret,
    }).encode()
    status, body = req("POST", "https://login.patty.io/realms/internal/protocol/openid-connect/token", data=tok_body)
    kc_tok = json.loads(body)["access_token"]
    status, body = req(
        "GET",
        "https://login.patty.io/admin/realms/internal/users?username=" + KEYCLOAK_USER,
        headers={"Authorization": f"Bearer {kc_tok}"},
    )
    user = json.loads(body)[0]
    attrs = user.get("attributes") or {}
    stored_pk = (attrs.get("nostr_pubkey") or [None])[0]
    stored_enc = (attrs.get("nostr_privkey_encrypted") or [None])[0]
    assert stored_pk == result["pubkey"], f"stored pubkey mismatch: {stored_pk}"
    assert stored_enc, "encrypted private key not persisted"
    print(f"    nostr_pubkey persisted ✓  encrypted key present ✓")

    # 6c. relay membership row
    step(6, "verify relay_members oidc row")
    kubectl_cmd = [
        "kubectl", "-n", "griddle", "exec", "griddle-postgresql-0", "--",
        "psql", "-U", "buzz", "-d", "buzz", "-t", "-A", "-c",
        f"SELECT added_by FROM relay_members WHERE pubkey='{result['pubkey']}';",
    ]
    out = subprocess.run(kubectl_cmd, capture_output=True, text=True).stdout.strip()
    assert out == "oidc", f"membership row missing or wrong added_by: {out!r}"
    print("    relay_members row added_by=oidc ✓")

    # 7. cleanup membership row so the test is repeatable
    subprocess.run([
        "kubectl", "-n", "griddle", "exec", "griddle-postgresql-0", "--",
        "psql", "-U", "buzz", "-d", "buzz", "-t", "-A", "-c",
        f"DELETE FROM relay_members WHERE pubkey='{result['pubkey']}' AND added_by='oidc';",
    ], capture_output=True, text=True)
    step(7, "cleanup done (oidc row removed; Keycloak attrs kept for re-login identity)")

    print("\nALL STEPS PASSED — full SSO flow works end-to-end.")


if __name__ == "__main__":
    sys.exit(main())
