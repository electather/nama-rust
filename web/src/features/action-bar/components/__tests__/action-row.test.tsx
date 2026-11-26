import { create } from "@bufbuild/protobuf";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { ActionRow } from "@/features/action-bar/components/action-row";
import { RequestSchema, RequestState } from "@/gen/nama/v1/request_pb";

afterEach(cleanup);

// The bar's selected row is this row ("office") or another one ("matrix").
test.each([
  { label: "Declined", state: RequestState.DECLINED, activeId: "matrix", selected: false },
  { label: "Declined", state: RequestState.DECLINED, activeId: "office", selected: true },
  { label: "Failed", state: RequestState.FAILED, activeId: "matrix", selected: false },
  { label: "Failed", state: RequestState.FAILED, activeId: "office", selected: true },
])(
  "a $label Request pill on the Popover tint sits in a row whose selection drops it (selected: $selected)",
  ({ label, state, activeId, selected }) => {
    const row = {
      id: "office",
      label: "Office Space",
      hint: "",
      request: create(RequestSchema, { state }),
    };
    const bar = { active: { id: activeId, label: "", hint: "" }, pick: () => {} };
    render(<ActionRow row={row} bar={bar} listId="list" />);
    const pill = screen.getByText(label);
    const has = (name: string) => pill.classList.contains(name);
    // jsdom applies no Tailwind, so the selector `group-aria-selected:` compiles to stands in for it.
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
