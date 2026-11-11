import { expect, test } from "@playwright/test";
import net from "node:net";

// The E2E transport itself (core/src/bin/dev_server.rs). The header's clinic.get used to stay
// unanswered: the old HTTP library queued a connection that the browser opened together with others
// while its threads sat in open keep-alive connections, and never read it. Here: connections that stay
// open and idle, then a burst of new ones at once; every request of the burst must be answered.

function connect(): Promise<net.Socket> {
  return new Promise((resolve, reject) => {
    const s = net.connect(8787, "127.0.0.1", () => resolve(s));
    s.once("error", reject);
  });
}

function healthOn(s: net.Socket): Promise<string> {
  return new Promise((resolve) => {
    let data = "";
    s.on("data", (c: { toString(): string }) => {
      data += c.toString();
      if (data.includes("\r\n\r\nok")) resolve(data.split("\r\n")[0]);
    });
    s.write("GET /health HTTP/1.1\r\nHost: 127.0.0.1:8787\r\n\r\n");
  });
}

test("dev server answers every connection of a burst while others stay open", async () => {
  const idle = await Promise.all(Array.from({ length: 8 }, connect));
  const burst = await Promise.all(Array.from({ length: 12 }, connect));
  try {
    const answers = await Promise.race([
      Promise.all(burst.map(healthOn)),
      new Promise<string[]>((r) => setTimeout(() => r(["NO ANSWER IN 3s"]), 3000)),
    ]);
    expect(answers).toEqual(Array(12).fill("HTTP/1.1 200 OK"));
  } finally {
    [...idle, ...burst].forEach((s) => s.destroy());
  }
});
