// Channels end to end over the REAL gateway: channel.list → channel.history → the live SSE stream
// → channel.post — REST + SSE + workspace scoping exercised for real (app-shell scope §Testing
// "SSE"). Includes the kill+resume story: the stream has no Last-Event-ID replay (the gateway emits
// no `id:` fields), so resume = reconnect + a `channel.history` catch-up read on `onOpen`.

import { describe, expect, it } from "vitest";

import type { ChannelRecord, Item } from "../src/index";
import { addMember, realClient, until } from "./harness";

describe("channels over REST + SSE", () => {
  it("lists_posts_reads_history_and_sees_a_second_clients_post_live", async () => {
    const app = realClient(); // the phone
    const web = realClient(); // a second client posting into the same room
    await app.login("ana", "app-chan-a");
    await addMember(app, "bob"); // ana bootstrapped as admin; admit bob before his login
    await web.login("bob", "app-chan-a");

    await app.invoke("channel_create", { channel: "general" });
    const rooms = await app.invoke<ChannelRecord[]>("channel_list");
    expect(rooms.map((c) => c.id)).toContain("general");

    await app.invoke("channel_post", {
      channel: "general",
      item: { id: "m1", channel: "general", author: "user:ana", body: "hello", ts: 1 },
    });
    const history = await app.invoke<Item[]>("channel_history", { channel: "general" });
    expect(history.map((i) => i.id)).toContain("m1");

    // Live: open the app's SSE stream, then post from the second client — the frame arrives.
    const seen: Item[] = [];
    let opened = 0;
    const stream = app.streamChannel("general", {
      onMessage: (i) => seen.push(i),
      onOpen: () => opened++,
    });
    await until(() => opened >= 1);
    await web.invoke("channel_post", {
      channel: "general",
      item: { id: "m2", channel: "general", author: "user:bob", body: "from web", ts: 2 },
    });
    await until(() => seen.find((i) => i.id === "m2"));
    stream.close();
  });

  it("kill_and_resume_closes_the_gap_via_history_catchup", async () => {
    const app = realClient();
    const web = realClient();
    await app.login("ana", "app-chan-b");
    await addMember(app, "bob");
    await web.login("bob", "app-chan-b");
    await app.invoke("channel_create", { channel: "ops" });

    // First connection, then kill it.
    let opened = 0;
    const live: Item[] = [];
    const first = app.streamChannel("ops", {
      onMessage: (i) => live.push(i),
      onOpen: () => opened++,
    });
    await until(() => opened >= 1);
    first.close();

    // A message lands WHILE the app is disconnected (backgrounded phone).
    await web.invoke("channel_post", {
      channel: "ops",
      item: { id: "gap", channel: "ops", author: "user:bob", body: "missed?", ts: 3 },
    });

    // Resume: reconnect and do the durable catch-up read on open — the gap message is recovered,
    // and the live feed works again after it.
    const caughtUp: Item[] = [];
    const second = app.streamChannel("ops", {
      onMessage: (i) => live.push(i),
      onOpen: () => {
        void app
          .invoke<Item[]>("channel_history", { channel: "ops" })
          .then((items) => caughtUp.push(...items));
      },
    });
    await until(() => caughtUp.find((i) => i.id === "gap"));
    await web.invoke("channel_post", {
      channel: "ops",
      item: { id: "after", channel: "ops", author: "user:bob", body: "back live", ts: 4 },
    });
    await until(() => live.find((i) => i.id === "after"));
    second.close();
  });
});
