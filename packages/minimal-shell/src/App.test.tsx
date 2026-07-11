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
});
