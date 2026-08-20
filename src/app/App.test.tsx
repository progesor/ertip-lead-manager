import { render, screen } from "@testing-library/react";
import { App } from "./App";

describe("App", () => {
  it("renders the desktop shell and default dashboard route", () => {
    render(<App />);

    expect(screen.getByText("Ertip Lead Manager")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Genel Bakış" })).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Ana menü" })).toBeTruthy();
  });
});
