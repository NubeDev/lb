// Vitest setup: jest-dom matchers + reset the in-memory node fakes between tests so each test starts
// from an empty store (the fakes are module-global by design). The collaboration fakes (workspace,
// channel registry, members, inbox, outbox) and the session are reset here too.
import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";

import { __resetFake } from "@/lib/ipc/fake";
import { __resetWorkspaceFake } from "@/lib/ipc/workspace.fake";
import { __resetChannelRegistryFake } from "@/lib/ipc/channelRegistry.fake";
import { __resetMembersFake } from "@/lib/ipc/members.fake";
import { __resetInboxFake } from "@/lib/ipc/inbox.fake";
import { __resetOutboxFake } from "@/lib/ipc/outbox.fake";
import { setSession } from "@/lib/session/session.store";

afterEach(() => {
  __resetFake();
  __resetWorkspaceFake();
  __resetChannelRegistryFake();
  __resetMembersFake();
  __resetInboxFake();
  __resetOutboxFake();
  setSession(null);
});
