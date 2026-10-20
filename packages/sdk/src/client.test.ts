// Written by Kone Tshivhinda & Claude
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { TimeLexClient, TimeLexError } from "./client.js";
import type { ActivityEvent } from "./types.js";

function serve(handler: Parameters<typeof createServer>[1]) {
  const server = createServer(handler);
  return new Promise<{ url: string; close: () => void }>((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address() as AddressInfo;
      resolve({ url: `http://127.0.0.1:${port}`, close: () => server.close() });
    });
  });
}

test("health and meta parse gateway responses and send the bearer token", async () => {
  let seenAuth: string | undefined;
  const srv = await serve((req, res) => {
    seenAuth = req.headers.authorization;
    res.setHeader("content-type", "application/json");
    if (req.url === "/health") res.end(JSON.stringify({ status: "ok" }));
    else if (req.url === "/v1/meta")
      res.end(
        JSON.stringify({
          name: "timelex-gateway",
          version: "0.1.0",
          contracts: { activityEvent: "1.0" },
          stage: "pre-alpha",
        }),
      );
    else {
      res.statusCode = 404;
      res.end("{}");
    }
  });
  try {
    const client = new TimeLexClient({ baseUrl: srv.url + "/", apiKey: "tl_test_key" });
    assert.deepEqual(await client.health(), { status: "ok" });
    const meta = await client.meta();
    assert.equal(meta.contracts.activityEvent, "1.0");
    assert.equal(seenAuth, "Bearer tl_test_key");
  } finally {
    srv.close();
  }
});

test("non-2xx responses throw TimeLexError with the status", async () => {
  const srv = await serve((_req, res) => {
    res.statusCode = 503;
    res.end("{}");
  });
  try {
    const client = new TimeLexClient({ baseUrl: srv.url });
    await assert.rejects(client.health(), (e: unknown) => {
      assert.ok(e instanceof TimeLexError);
      assert.equal(e.status, 503);
      return true;
    });
  } finally {
    srv.close();
  }
});

test("golden contract example has the keys the TypeScript type expects", () => {
  const path = fileURLToPath(
    new URL("../../../contracts/examples/activity-event.json", import.meta.url),
  );
  const event = JSON.parse(readFileSync(path, "utf8")) as ActivityEvent;
  for (const key of ["schemaVersion", "id", "tenantId", "source", "kind", "actor", "privacy"]) {
    assert.ok(key in event, `missing ${key}`);
  }
  assert.match(event.schemaVersion, /^1\.\d+$/);
});
