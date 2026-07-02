// QueryTab ext-widget path (thecrew findings 7+8, restored in the viz panel-editor). The concurrent
// panel-editor rework dropped the "Extension widgets" PickerGroup, so a packaged `[[widget]]` could not
// be added through the live builder. These tests pin the restored behaviour with no gateway (the hooks
// are mocked to a known roster): the group is offered, selecting a tile makes a `view:"ext:…"` cell with
// NO target, and a Scene tile surfaces a scene picker over `assets.list_docs` that sets `options.sceneId`.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { QueryTab } from "./QueryTab";
import type { EditorState } from "../cellEditorState";
import type { SourceEntry } from "../../builder/sourcePicker";

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

vi.mock("../../builder/useSourcePicker", () => ({
  useSourcePicker: () => ({ entries: [SERIES_ENTRY, WIDGET_ENTRY], installed: [], loading: false }),
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
  it("offers the packaged tile in the source picker's 'Extension widgets' group", () => {
    render(<QueryTab ws="acme" state={baseState({})} patch={() => {}} />);
    // The restored optgroup + its packaged-tile option are present in the source select.
    const opt = screen.getByRole("option", { name: "thecrew · Scene" }) as HTMLOptionElement;
    expect(opt).toBeTruthy();
    expect(opt.closest("optgroup")?.label).toBe("Extension widgets");
  });

  it("selecting a tile sets the cell view to its ext key and clears any target", () => {
    const patch = vi.fn();
    render(<QueryTab ws="acme" state={baseState({})} patch={patch} />);
    fireEvent.change(screen.getByLabelText("panel source"), { target: { value: "widget:thecrew/scene" } });
    expect(patch).toHaveBeenCalledWith(
      expect.objectContaining({ view: "ext:thecrew/scene", targets: [], sql: undefined }),
    );
  });

  it("leaving a widget view for a real source drops the ext view", () => {
    const patch = vi.fn();
    render(<QueryTab ws="acme" state={baseState({ view: "ext:thecrew/scene" })} patch={patch} />);
    fireEvent.change(screen.getByLabelText("panel source"), { target: { value: "series:ahu1.speed" } });
    // First patch clears the ext view; a later patch sets the series target.
    expect(patch).toHaveBeenCalledWith(expect.objectContaining({ view: "" }));
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
