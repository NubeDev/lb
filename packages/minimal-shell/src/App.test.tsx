import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { App } from "./App";

describe("MinimalShell", () => {
  it("renders the login view when no session", () => {
    localStorage.removeItem("lb.session");
    render(<App />);
    expect(screen.getByRole("heading", { name: "Sign in" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sign in" })).toBeTruthy();
  });

  it("renders input fields for user and workspace", () => {
    localStorage.removeItem("lb.session");
    render(<App />);
    expect(screen.getByPlaceholderText("user")).toBeTruthy();
    expect(screen.getByPlaceholderText("workspace")).toBeTruthy();
  });

  // Regression: getSession used to JSON.parse a fresh object per call — useSyncExternalStore
  // then sees a new snapshot every render and React loops once a session exists.
  it("renders with a stored session without a getSnapshot loop", () => {
    localStorage.setItem(
      "lb.session",
      JSON.stringify({ token: "t", principal: "user:a", workspace: "ws-a" }),
    );
    render(<App />);
    // Logged in: no login heading; the ext mount path renders (loading or discovery error).
    expect(screen.queryByRole("heading", { name: "Sign in" })).toBeNull();
    localStorage.removeItem("lb.session");
  });

  // Regression: a gateway 401 clears localStorage and fires lb.session.cleared; the session
  // store must re-emit so the UI drops to the login view without a reload.
  it("drops to the login view when the session is cleared by a 401", async () => {
    localStorage.setItem(
      "lb.session",
      JSON.stringify({ token: "t", principal: "user:a", workspace: "ws-a" }),
    );
    render(<App />);
    expect(screen.queryByRole("heading", { name: "Sign in" })).toBeNull();
    localStorage.removeItem("lb.session");
    window.dispatchEvent(new Event("lb.session.cleared"));
    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeTruthy();
  });
});
