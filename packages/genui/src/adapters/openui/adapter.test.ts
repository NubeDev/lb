// OpenUI Lang → IR round-trips + streaming partial-input (genui-scope Testing plan: Lang→IR round-trips;
// streaming partial-input renders, mid-line, forward refs).
import { describe, it, expect } from "vitest";
import { nubeCatalog } from "../../catalog/nubeCatalog";
import { parseLang } from "./parse";
import { createLangStream } from "./stream";

describe("parseLang → IR", () => {
  it("lowers a statement-based Lang doc to a flat id-referenced IR map", () => {
    const lang = ['a = Stat("Temp", 21.5)', 'b = Text("hi")', "root = Stack(\"vertical\", [a, b])"].join("\n");
    const { ir } = parseLang(lang, nubeCatalog);
    expect(ir.surface.root).toBe("root");
    expect(Object.keys(ir.components).sort()).toEqual(["a", "b", "root"]);
    expect(ir.components.root.component).toBe("stack");
    expect(ir.components.root.children).toEqual(["a", "b"]);
    expect(ir.components.a.component).toBe("stat");
    expect(ir.components.b.component).toBe("text");
  });

  it("maps a data $bind object prop straight through", () => {
    const lang = 'root = Stat({"$bind": "/data/A/value"}, "Count")';
    const { ir } = parseLang(lang, nubeCatalog);
    expect(ir.components.root.props?.value).toEqual({ $bind: "/data/A/value" });
  });

  it("translates lang PascalCase names back to catalog lowercase (incl. multi-word)", () => {
    const lang = ['c = TimeSeries({"$bind":"/data/B/rows"})', "root = Card(\"Room\", [c])"].join("\n");
    const { ir } = parseLang(lang, nubeCatalog);
    expect(ir.components.c.component).toBe("timeseries");
    expect(ir.components.root.component).toBe("card");
  });
});

describe("createLangStream (streaming)", () => {
  it("renders something stable through mid-stream partial input, resolving forward refs as lines land", () => {
    const stream = createLangStream(nubeCatalog);
    // A forward ref: `root` references `a`/`b` before they're defined — resolves as the lines arrive.
    stream.push('root = Stack("vertical", [a, b])\n');
    stream.push('a = Stat("Temp", 20)\n');
    const mid = stream.push('b = Text("hel'); // mid-line, incomplete
    expect(mid.surface.surfaceId).toBe("cell"); // never throws mid-stream
    const done = stream.push('lo")\n');
    expect(done.surface.root).toBe("root");
    expect(done.components.root.children).toEqual(["a", "b"]);
    expect(done.components.b.component).toBe("text");
  });

  it("set() diffs the full accumulated buffer", () => {
    const stream = createLangStream(nubeCatalog);
    const r = stream.set(['a = Text("x")', "root = Stack(\"vertical\", [a])"].join("\n"));
    expect(r.components.root.children).toEqual(["a"]);
  });
});
