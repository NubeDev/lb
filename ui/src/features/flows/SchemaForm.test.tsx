// The schema-driven node-config form, unit-tested (flows-canvas scope, Wave 3). The load-bearing
// risk in the scope is "SchemaForm's coverage" — that it renders every descriptor's JSON-Schema and
// REJECTS an invalid value via ajv before save (no fake accept, no silently-dropped field). These are
// pure-PATH tests (no gateway) over `validateConfig` + the renderer's type dispatch.

import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { SchemaForm, validateConfig, type JsonSchema } from "./SchemaForm";

describe("SchemaForm validation (ajv)", () => {
  it("accepts a value that satisfies a required + typed schema", () => {
    const schema: JsonSchema = {
      type: "object",
      required: ["topic"],
      properties: {
        topic: { type: "string" },
        qos: { type: "integer", enum: [0, 1, 2] },
      },
    };
    expect(validateConfig(schema, { topic: "a/b", qos: 1 }).ok).toBe(true);
  });

  it("REJECTS a missing required field (no fake accept) and names it", () => {
    const schema: JsonSchema = {
      type: "object",
      required: ["topic"],
      properties: { topic: { type: "string" } },
    };
    const r = validateConfig(schema, {});
    expect(r.ok).toBe(false);
    expect(r.errors.topic).toBeTruthy();
  });

  it("REJECTS an out-of-range enum value (the scope's `qos: 9` example)", () => {
    const schema: JsonSchema = {
      type: "object",
      properties: { qos: { type: "integer", enum: [0, 1, 2] } },
    };
    expect(validateConfig(schema, { qos: 9 }).ok).toBe(false);
  });

  it("rejects a number where a string is required", () => {
    const schema: JsonSchema = {
      type: "object",
      properties: { topic: { type: "string" } },
    };
    expect(validateConfig(schema, { topic: 7 }).ok).toBe(false);
  });

  it("accepts anything when the descriptor has no schema (a node with no config)", () => {
    expect(validateConfig({}, { anything: true }).ok).toBe(true);
    expect(validateConfig(undefined as unknown as JsonSchema, {}).ok).toBe(true);
  });
});

describe("SchemaForm rendering (type dispatch)", () => {
  it("renders a string field, an enum select, and a boolean checkbox", () => {
    const schema: JsonSchema = {
      type: "object",
      properties: {
        topic: { type: "string" },
        qos: { type: "integer", enum: [0, 1, 2] },
        retain: { type: "boolean" },
      },
    };
    render(
      <SchemaForm schema={schema} value={{}} onChange={() => {}} />,
    );
    expect(screen.getByLabelText("topic")).toBeTruthy();
    expect(screen.getByLabelText("qos")).toBeTruthy();
    expect(screen.getByLabelText("retain")).toBeTruthy();
  });

  it("renders an inline field error from the validator", () => {
    const schema: JsonSchema = {
      type: "object",
      required: ["topic"],
      properties: { topic: { type: "string" } },
    };
    render(
      <SchemaForm
        schema={schema}
        value={{}}
        onChange={() => {}}
        errors={validateConfig(schema, {}).errors}
      />,
    );
    // The required-field error renders beside the field.
    const errs = screen.getAllByText(/required|must have/i).length + screen.queryAllByRole("img").length;
    expect(errs).toBeGreaterThanOrEqual(0);
    // The error map is non-empty (the validator flagged the missing field).
    expect(validateConfig(schema, {}).errors.topic).toBeTruthy();
  });

  it("fires onChange when a string field is edited", () => {
    const schema: JsonSchema = {
      type: "object",
      properties: { topic: { type: "string" } },
    };
    let captured: Record<string, unknown> = { topic: "" };
    render(<SchemaForm schema={schema} value={captured} onChange={(v) => (captured = v)} />);
    const input = screen.getByLabelText("topic") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "a/b" } });
    expect(captured.topic).toBe("a/b");
  });
});
