import assert from "node:assert/strict";
import test from "node:test";

import { isChannelLink, parseChannelLink } from "./channelLink.ts";

const CHANNEL_ID = "580ca78b-9dae-46f3-8854-bd671853ba32";
const MESSAGE_ID =
  "8455293f0123456789abcdef0123456789abcdef0123456789abcdef01234567";

test("parseChannelLink accepts the canonical channel path", () => {
  assert.deepEqual(parseChannelLink(`crew://channel/${CHANNEL_ID}`), {
    ok: true,
    value: { channelId: CHANNEL_ID },
  });
});

test("parseChannelLink accepts a channel message path", () => {
  assert.deepEqual(
    parseChannelLink(`crew://channel/${CHANNEL_ID}/${MESSAGE_ID}`),
    {
      ok: true,
      value: { channelId: CHANNEL_ID, messageId: MESSAGE_ID },
    },
  );
});

test("parseChannelLink accepts v7 and canonicalizes uppercase UUIDs", () => {
  assert.deepEqual(
    parseChannelLink("crew://channel/018fdb5d-3a64-7c35-b5f9-4a23e1f9d2d9"),
    {
      ok: true,
      value: { channelId: "018fdb5d-3a64-7c35-b5f9-4a23e1f9d2d9" },
    },
  );
  assert.deepEqual(
    parseChannelLink("crew://channel/580CA78B-9DAE-46F3-8854-BD671853BA32"),
    {
      ok: true,
      value: { channelId: "580ca78b-9dae-46f3-8854-bd671853ba32" },
    },
  );
});

test("parseChannelLink rejects malformed channel links", () => {
  for (const href of [
    "crew://channel",
    "crew://channel/",
    "crew://channel/one/two",
    `crew://channel/${CHANNEL_ID}/not-hex`,
    `crew://channel/${CHANNEL_ID}/${"a".repeat(63)}`,
    `crew://channel/${CHANNEL_ID}/${MESSAGE_ID}/extra`,
    `crew://channel/${CHANNEL_ID}/`,
    "crew://channel/one?extra=true",
    "crew://channel/one#fragment",
    "https://channel/one",
    "crew://channel/not-a-uuid",
    "crew://channel/%",
    "crew://channel/%ZZ",
    "crew://channel/%2F",
    "crew://channel/%00",
  ]) {
    assert.equal(parseChannelLink(href).ok, false, href);
  }
});

test("isChannelLink recognizes only a valid canonical link", () => {
  assert.equal(
    isChannelLink("crew://channel/580ca78b-9dae-46f3-8854-bd671853ba32"),
    true,
  );
  assert.equal(
    isChannelLink("crew://message?channel=channel-1&id=message-1"),
    false,
  );
});
