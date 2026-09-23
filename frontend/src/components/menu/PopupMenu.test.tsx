// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { PopupMenu } from "./PopupMenu";

afterEach(cleanup);

// Pressing the control that opened a menu closed it on pointer-down, and the click that
// followed opened it again: a title-bar menu could not be closed by clicking its title.
it("pressing the opener closes the menu without reopening it", () => {
  const opener = document.createElement("button");
  const reopen = vi.fn();
  opener.addEventListener("click", reopen);
  document.body.append(opener);
  const onClose = vi.fn();
  render(
    <PopupMenu
      items={[{ label: "New session", onSelect: () => {} }]}
      x={0}
      y={0}
      onClose={onClose}
      anchor={opener}
    />,
  );
  opener.dispatchEvent(new Event("pointerdown", { bubbles: true }));
  opener.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  expect(onClose).toHaveBeenCalledTimes(1);
  expect(reopen).not.toHaveBeenCalled();
  // Only that one click: the next is the person's.
  opener.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  expect(reopen).toHaveBeenCalledTimes(1);
  opener.remove();
});

it("a press elsewhere closes the menu and lets the click through", () => {
  const other = document.createElement("button");
  const clicked = vi.fn();
  other.addEventListener("click", clicked);
  document.body.append(other);
  const onClose = vi.fn();
  render(
    <PopupMenu
      items={[{ label: "New session", onSelect: () => {} }]}
      x={0}
      y={0}
      onClose={onClose}
      anchor={document.createElement("span")}
    />,
  );
  other.dispatchEvent(new Event("pointerdown", { bubbles: true }));
  other.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  expect(onClose).toHaveBeenCalledTimes(1);
  expect(clicked).toHaveBeenCalledTimes(1);
  other.remove();
});
