import { create } from "@bufbuild/protobuf";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RowText } from "@/features/action-bar/components/row-text";
import { RequestSchema, RequestState } from "@/gen/nama/v1/request_pb";

afterEach(cleanup);

test.each([
  ["Declined", RequestState.DECLINED, false],
  ["Declined", RequestState.DECLINED, true],
  ["Failed", RequestState.FAILED, false],
  ["Failed", RequestState.FAILED, true],
])(
  "a row's %s Request pill is red on the 10% tint over Popover, untinted on Accent (selected: %s)",
  (label, state, selected) => {
    const request = create(RequestSchema, { state });
    // The row is the `group` whose `aria-selected` drops the tint (`ActionRow`).
    render(
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- an <option> can't hold the row's pill, as in `ActionRow`
      <div role="option" aria-selected={selected} className="group">
        <RowText row={{ id: "office", label: "Office Space", hint: "", request }} />
      </div>,
    );
    const pill = screen.getByText(label);
    expect({
      selected: pill.closest(".group")?.getAttribute("aria-selected"),
      "bg-destructive/10": pill.classList.contains("bg-destructive/10"),
      "bg-destructive/15": pill.classList.contains("bg-destructive/15"),
      "group-aria-selected:bg-transparent": pill.classList.contains(
        "group-aria-selected:bg-transparent",
      ),
      "text-destructive": pill.classList.contains("text-destructive"),
    }).toStrictEqual({
      selected: String(selected),
      "bg-destructive/10": true,
      "bg-destructive/15": false,
      "group-aria-selected:bg-transparent": true,
      "text-destructive": true,
    });
  },
);
