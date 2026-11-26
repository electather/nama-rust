import { create } from "@bufbuild/protobuf";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RowText } from "@/features/action-bar/components/row-text";
import { RequestSchema, RequestState } from "@/gen/nama/v1/request_pb";

afterEach(cleanup);

test.each([
  ["Declined", false, RequestState.DECLINED],
  ["Declined", true, RequestState.DECLINED],
  ["Failed", false, RequestState.FAILED],
  ["Failed", true, RequestState.FAILED],
])(
  "a row's %s Request pill keeps its Popover tint unless its row is selected (selected: %s)",
  (label, selected, state) => {
    const request = create(RequestSchema, { state });
    // jsdom applies no Tailwind, so the selector `group-aria-selected:` compiles to stands in for it.
    render(
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- an <option> can't hold the row's pill, as in `ActionRow`
      <div role="option" aria-selected={selected} className="group">
        <RowText row={{ id: "office", label: "Office Space", hint: "", request }} />
      </div>,
    );
    const pill = screen.getByText(label);
    const has = (name: string) => pill.classList.contains(name);
    expect({
      "bg-destructive/10": has("bg-destructive/10"),
      "bg-destructive/15": has("bg-destructive/15"),
      "text-destructive": has("text-destructive"),
      "group-aria-selected:bg-transparent": has("group-aria-selected:bg-transparent"),
      "in a selected group": pill.matches('.group[aria-selected="true"] *'),
    }).toStrictEqual({
      "bg-destructive/10": true,
      "bg-destructive/15": false,
      "text-destructive": true,
      "group-aria-selected:bg-transparent": true,
      "in a selected group": selected,
    });
  },
);
