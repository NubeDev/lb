// Unit test for the cron arg widget (channel rich responses scope). NO gateway. The antd-based
// `react-js-cron` builder needs a full DOM (matchMedia etc.) that jsdom doesn't provide, so we stub the
// external `Cron` component to a plain input — this is a true EXTERNAL UI lib (not node behavior), so a
// thin stub is allowed (rule 9). We assert CronArg's CONTRACT: an incoming `value` reaches the builder
// and an edit round-trips back out through `onChange` (value in → onChange out).

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

// Stub react-js-cron: a controlled text input that mirrors the `{ value, setValue }` contract the real
// builder exposes — enough to prove CronArg wires value in / onChange out.
vi.mock("react-js-cron", () => ({
  Cron: ({ value, setValue }: { value: string; setValue: (v: string) => void }) => (
    <input aria-label="cron-stub" value={value} onChange={(e) => setValue(e.target.value)} />
  ),
}));
// The builder imports its CSS; stub it so the import resolves under jsdom.
vi.mock("react-js-cron/dist/styles.css", () => ({}));

import { CronArg } from "./CronArg";

describe("CronArg widget", () => {
  it("round-trips a cron string (value in → onChange out)", async () => {
    const onChange = vi.fn();
    render(<CronArg value="0 8 * * 1" onChange={onChange} />);

    // Value in: the incoming cron string reaches the builder.
    const input = (await screen.findByLabelText("cron-stub")) as HTMLInputElement;
    expect(input.value).toBe("0 8 * * 1");

    // onChange out: an edit is reported back up verbatim (lossless round-trip).
    const user = userEvent.setup();
    await user.type(input, "5");
    expect(onChange).toHaveBeenLastCalledWith("0 8 * * 15");
  });

  it("defaults an empty value to a runnable daily cron (never a blank builder)", async () => {
    render(<CronArg value="" onChange={vi.fn()} />);
    const input = (await screen.findByLabelText("cron-stub")) as HTMLInputElement;
    expect(input.value).toBe("0 9 * * *");
  });
});
