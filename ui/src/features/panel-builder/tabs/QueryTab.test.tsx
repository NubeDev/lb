// QueryTab ext-widget path (thecrew findings 7+8, restored in the viz panel-editor). The concurrent
// panel-editor rework dropped the "Extension widgets" PickerGroup, so a packaged `[[widget]]` could not
// be added through the live builder. These tests pin the restored behaviour with no gateway (the hooks
// are mocked to a known roster): the group is offered, selecting a tile makes a `view:"ext:…"` cell with
// NO target, and a Scene tile surfaces a scene picker over `assets.list_docs` that sets `options.sceneId`.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { QueryTab } from "./QueryTab";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import type { SourceEntry } from "@/features/dashboard/builder/sourcePicker";

// --- mocked picker data: one series + one packaged Scene tile (thecrew · Scene). ---
const WIDGET_ENTRY: SourceEntry = {
  id: "widget:thecrew/scene",
  group: "widget",
  label: "thecrew · Scene",
  icon: "box",
  viewKey: "ext:thecrew/scene",
  writes: false,
};
const SERIES_ENTRY: SourceEntry = {
  id: "series:ahu1.speed",
  group: "series",
  label: "ahu1.speed",
  source: { tool: "series.read", args: { series: "ahu1.speed" } },
  writes: false,
};
// A rule with two declared params (one labelled, one bare) → its Query-tab params form.
const RULE_ENTRY: SourceEntry = {
  id: "rule:by-site",
  group: "rules",
  label: "By site",
  source: { tool: "rules.run", args: { rule_id: "by-site" } },
  writes: false,
  params: [{ name: "site", label: "Site" }, { name: "hours" }],
};
// A rule with NO params → no form.
const RULE_NOPARAMS: SourceEntry = {
  id: "rule:plain",
  group: "rules",
  label: "Plain",
  source: { tool: "rules.run", args: { rule_id: "plain" } },
  writes: false,
  params: [],
};

// A rule with a NUMBER param → coercion to a JSON number.
const RULE_NUMBER: SourceEntry = {
  id: "rule:nrule",
  group: "rules",
  label: "N",
  source: { tool: "rules.run", args: { rule_id: "nrule" } },
  writes: false,
  params: [{ name: "hours", kind: "number" }],
};
// A rule with an ENUM param → a <select> of its options.
const RULE_ENUM: SourceEntry = {
  id: "rule:erule",
  group: "rules",
  label: "E",
  source: { tool: "rules.run", args: { rule_id: "erule" } },
  writes: false,
  params: [{ name: "region", kind: "enum", options: ["emea", "amer"] }],
};

vi.mock("@/features/dashboard/builder/useSourcePicker", () => ({
  useSourcePicker: () => ({
    entries: [SERIES_ENTRY, WIDGET_ENTRY, RULE_ENTRY, RULE_NOPARAMS, RULE_NUMBER, RULE_ENUM],
    installed: [],
    loading: false,
  }),
}));
vi.mock("./useDatasourceList", () => ({
  useDatasourceList: () => ({
    options: [
      { type: "surreal", label: "SurrealDB (native)" },
      { type: "series", label: "Series" },
    ],
    loading: false,
  }),
  refForOption: (o: { type: string }) => ({ type: o.type }),
}));
const listScenes = vi.fn();
vi.mock("./useSceneDocs", () => ({
  useSceneDocs: () => listScenes(),
}));

function baseState(over: Partial<EditorState>): EditorState {
  return {
    view: "",
    title: "",
    description: "",
    targets: [],
    options: {},
    transformations: [],
    carry: {
      v: 2,
      widget_type: "chart",
      binding: { series: "" },
      action: undefined,
      pluginVersion: undefined,
      targetRepr: "none",
      extraOptions: {},
    },
    ...over,
  } as EditorState;
}

beforeEach(() => {
  listScenes.mockReturnValue({ scenes: [], loading: false });
});

describe("QueryTab — Extension widgets group (finding 7)", () => {
  // The source picker is now a SEARCHABLE combobox (data-studio-ux): focus the input to open the list,
  // type to filter, then mouse-down the option (onMouseDown fires before the input blur closes the list).
  const openPicker = () => fireEvent.focus(screen.getByLabelText("panel source"));
  const pick = (name: string | RegExp) =>
    fireEvent.mouseDown(screen.getByRole("option", { name }));

  it("offers the packaged tile in the source picker's 'Extension widgets' group", () => {
    render(<QueryTab ws="acme" state={baseState({})} patch={() => {}} />);
    openPicker();
    // The packaged-tile option is present, under its "Extension widgets" group header.
    const opt = screen.getByRole("option", { name: "thecrew · Scene" });
    expect(opt).toBeTruthy();
    // The group header sits directly above the first option of that group.
    expect(screen.getByText("Extension widgets")).toBeTruthy();
  });

  it("selecting a tile sets the cell view to its ext key and clears any target", () => {
    const patch = vi.fn();
    render(<QueryTab ws="acme" state={baseState({})} patch={patch} />);
    openPicker();
    pick("thecrew · Scene");
    expect(patch).toHaveBeenCalledWith(
      expect.objectContaining({ view: "ext:thecrew/scene", targets: [], sql: undefined }),
    );
  });

  it("leaving a widget view for a real source drops the ext view", () => {
    const patch = vi.fn();
    render(<QueryTab ws="acme" state={baseState({ view: "ext:thecrew/scene" })} patch={patch} />);
    openPicker();
    pick(/ahu1\.speed/);
    // First patch clears the ext view; a later patch sets the series target.
    expect(patch).toHaveBeenCalledWith(expect.objectContaining({ view: "" }));
  });
});

describe("QueryTab — rule params form (rules-as-source, open Q1)", () => {
  // A saved rule target, keyed by rule_id (seedEntryId disambiguates rules by rule_id, not just tool).
  const ruleState = (params?: Record<string, unknown>) =>
    baseState({
      targets: [
        {
          refId: "A",
          tool: "rules.run",
          args: { rule_id: "by-site", ...(params ? { params } : {}) },
          datasource: { type: "series" },
        },
      ],
    });

  it("renders one input per declared param when a rule with params is bound", () => {
    render(<QueryTab ws="acme" state={ruleState()} patch={() => {}} />);
    expect(screen.getByLabelText("rule param site")).toBeTruthy();
    expect(screen.getByLabelText("rule param hours")).toBeTruthy();
    // The labelled param shows its human label.
    expect(screen.getByText("Site")).toBeTruthy();
  });

  it("writes an edited param into the target's args.params", () => {
    const patch = vi.fn();
    render(<QueryTab ws="acme" state={ruleState()} patch={patch} />);
    fireEvent.change(screen.getByLabelText("rule param site"), { target: { value: "acme-hq" } });
    expect(patch).toHaveBeenCalledWith({
      targets: [
        expect.objectContaining({
          tool: "rules.run",
          args: { rule_id: "by-site", params: { site: "acme-hq" } },
        }),
      ],
    });
  });

  it("clearing a param omits it (rule sees an absent param, not empty string)", () => {
    const patch = vi.fn();
    render(<QueryTab ws="acme" state={ruleState({ site: "x" })} patch={patch} />);
    fireEvent.change(screen.getByLabelText("rule param site"), { target: { value: "" } });
    expect(patch).toHaveBeenCalledWith({
      targets: [expect.objectContaining({ args: { rule_id: "by-site", params: {} } })],
    });
  });

  it("coerces a NUMBER param to a JSON number (the cage sees a rhai number)", () => {
    const patch = vi.fn();
    // A rule whose param is typed number.
    render(
      <QueryTab
        ws="acme"
        state={baseState({
          targets: [{ refId: "A", tool: "rules.run", args: { rule_id: "nrule" }, datasource: { type: "series" } }],
        })}
        patch={patch}
      />,
    );
    // NRULE_ENTRY carries a number param `hours`.
    fireEvent.change(screen.getByLabelText("rule param hours"), { target: { value: "24" } });
    expect(patch).toHaveBeenCalledWith({
      targets: [expect.objectContaining({ args: { rule_id: "nrule", params: { hours: 24 } } })],
    });
  });

  it("renders an enum param as a select of its options", () => {
    render(
      <QueryTab
        ws="acme"
        state={baseState({
          targets: [{ refId: "A", tool: "rules.run", args: { rule_id: "erule" }, datasource: { type: "series" } }],
        })}
        patch={() => {}}
      />,
    );
    const sel = screen.getByLabelText("rule param region") as HTMLSelectElement;
    expect(sel.tagName).toBe("SELECT");
    expect(screen.getByRole("option", { name: "emea" })).toBeTruthy();
  });

  it("shows NO params form for a rule that declares none", () => {
    render(
      <QueryTab
        ws="acme"
        state={baseState({
          targets: [{ refId: "A", tool: "rules.run", args: { rule_id: "plain" }, datasource: { type: "series" } }],
        })}
        patch={() => {}}
      />,
    );
    expect(screen.queryByLabelText("rule params")).toBeNull();
  });
});

describe("QueryTab — Scene options field (finding 8)", () => {
  it("shows a scene picker for a Scene tile and sets options.sceneId on pick", () => {
    listScenes.mockReturnValue({
      scenes: [{ id: "scene:ahu-1", title: "AHU-1" }],
      loading: false,
    });
    const patch = vi.fn();
    render(<QueryTab ws="acme" state={baseState({ view: "ext:thecrew/scene" })} patch={patch} />);
    const picker = screen.getByLabelText("scene doc") as HTMLSelectElement;
    expect(screen.getByRole("option", { name: "AHU-1" })).toBeTruthy();
    fireEvent.change(picker, { target: { value: "scene:ahu-1" } });
    expect(patch).toHaveBeenCalledWith({ options: { sceneId: "scene:ahu-1" } });
  });

  it("does NOT show the scene field for a non-scene widget view", () => {
    render(<QueryTab ws="acme" state={baseState({ view: "ext:thecrew/other" })} patch={() => {}} />);
    expect(screen.queryByLabelText("scene doc")).toBeNull();
  });
});
