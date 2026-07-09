// The sqlite DB-file picker in the Add-datasource form, driven against a REAL spawned gateway
// (CLAUDE §9 / testing §0 — no fake fs, no `*.fake.ts`). The picker is the shared `HostPathPicker`
// engine in "file" mode: it anchors at the node HOME dir (`host.fs.home`) — because a sqlite DB is a
// plain node-local file that lives anywhere (e.g. /var/lib/lb/…), NOT under the devkit tree — and
// walks it one level at a time via the real `host.fs.list` verb. We seed a REAL `.db` file through the
// real `devkit.write_file` verb (which returns its canonical ABSOLUTE path), then browse from `/` down
// to it, assert it is a selectable entry that fills the DSN with its absolute node path when clicked,
// and that a non-db sibling is shown but NOT selectable. Proves the extension-page file picker is
// reused for the datasource path selection end to end.

import { beforeAll, describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { AddDatasourceForm } from "./AddDatasourceForm";
import { hostHomeDir } from "@/lib/host/fs.api";
import { invoke } from "@/lib/ipc/invoke";
import { signInWithCaps, useRealGateway } from "@/test/gateway-session";

let n = 0;

beforeAll(() => useRealGateway());

interface WriteReport {
  path: string;
}

/** Seed one file under the devkit root via the real `devkit.write_file` verb; returns its abs path. */
async function seedFile(rel: string, content: string): Promise<string> {
  const r = await invoke<WriteReport>("mcp_call", {
    tool: "devkit.write_file",
    args: { path: rel, content },
  });
  return r.path;
}

describe("AddDatasourceForm sqlite picker (real gateway)", () => {
  it("browses the node from the home dir and selects a real .db file for the DSN", async () => {
    const user = userEvent.setup();
    const ws = `ds-pick-${n++}`;
    await signInWithCaps("user:ada", ws, [
      "mcp:devkit.write_file:call",
      "mcp:host.fs.list:call",
      "mcp:host.fs.home:call",
    ]);

    // A real db file and a non-db sibling, both in the same seeded folder. We get their absolute
    // paths back and drive the picker down to them from the filesystem root.
    const dir = `ds-pick-${Date.now()}`;
    const dbPath = await seedFile(`${dir}/buildings.db`, "SQLite format 3 ");
    await seedFile(`${dir}/notes.txt`, "not a database");

    // The picker starts at the node home dir; the devkit root (where we seeded) is under it, so we
    // walk the segments BELOW home down to the seeded folder.
    const home = (await hostHomeDir()).path;
    expect(dbPath.startsWith(`${home}/`)).toBe(true);

    render(<AddDatasourceForm onAdd={() => {}} />);

    await user.selectOptions(screen.getByLabelText("datasource kind"), "sqlite");
    await user.click(screen.getByRole("button", { name: "browse for db file" }));

    // Scope entry lookups to the listing region (the breadcrumb also renders segment buttons).
    const entries = () => within(screen.getByLabelText("directory entries"));

    const belowHome = dbPath.slice(home.length + 1).split("/").slice(0, -1); // drop the filename
    for (const seg of belowHome) {
      await user.click(await entries().findByRole("button", { name: seg }));
    }

    const dbBtn = await entries().findByRole("button", { name: /buildings\.db/ });
    // The non-db sibling renders as static text, never a button.
    expect(entries().queryByRole("button", { name: /notes\.txt/ })).toBeNull();
    expect(entries().getByText("notes.txt")).toBeInTheDocument();

    await user.click(dbBtn);

    // Clicking the file fills the DSN with its absolute node path.
    const dsn = screen.getByLabelText("datasource dsn") as HTMLInputElement;
    expect(dsn.value).toBe(dbPath);
  });
});
