import { create } from "@bufbuild/protobuf";
import { createRouterTransport } from "@connectrpc/connect";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { ActionRow } from "@/features/action-bar/components/action-row";
import type { Row } from "@/features/action-bar/lib/rows";
import { RequestSchema, RequestState } from "@/gen/nama/v1/request_pb";

afterEach(cleanup);

// The bar's selected row is this row ("office") or another one ("matrix").
test.each([
  { activeId: "matrix", selected: false },
  { activeId: "office", selected: true },
])(
  "a Failed Request pill on the Popover tint sits in a row whose selection drops it (selected: $selected)",
  ({ activeId, selected }) => {
    const request = create(RequestSchema, { state: RequestState.FAILED });
    const row: Row = {
      id: "office",
      label: "Office Space",
      hint: "",
      standing: { kind: "stalled", request, can: "nothing" },
    };
    const bar = {
      active: { id: activeId, label: "", hint: "" },
      pick: () => {},
      // On a Server's own web app, every row is on the page's own Server.
      serverOf: () => ({ server: undefined, transport: createRouterTransport(() => {}) }),
    };
    render(<ActionRow row={row} bar={bar} listId="list" />);
    const pill = screen.getByText("Failed");
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
