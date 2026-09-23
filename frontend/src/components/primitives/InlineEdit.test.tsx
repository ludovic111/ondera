// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { InlineEdit } from "./InlineEdit";

afterEach(cleanup);

// The Enter that confirms an input method's composition renamed the track mid-word.
it("Enter while composing text does not commit", () => {
  const onCommit = vi.fn();
  const { getByRole } = render(
    <InlineEdit value="Keys" onCommit={onCommit} onCancel={() => {}} />,
  );
  const field = getByRole("textbox");
  fireEvent.change(field, { target: { value: "きー" } });
  fireEvent.keyDown(field, { key: "Enter", isComposing: true });
  expect(onCommit).not.toHaveBeenCalled();
  fireEvent.keyDown(field, { key: "Enter" });
  expect(onCommit).toHaveBeenCalledWith("きー");
});
