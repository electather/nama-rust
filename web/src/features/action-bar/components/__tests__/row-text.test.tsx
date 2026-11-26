import { create } from "@bufbuild/protobuf";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RowText } from "@/features/action-bar/components/row-text";
import { RequestSchema, RequestState } from "@/gen/nama/v1/request_pb";

afterEach(cleanup);

test.each([
  ["Declined", RequestState.DECLINED],
  ["Failed", RequestState.FAILED],
])("a row's %s Request pill is red on the 10% tint over Popover", (label, state) => {
  const request = create(RequestSchema, { state });
  render(<RowText row={{ id: "office", label: "Office Space", hint: "", request }} />);
  const pill = screen.getByText(label);
  expect({
    "bg-destructive/10": pill.classList.contains("bg-destructive/10"),
    "bg-destructive/15": pill.classList.contains("bg-destructive/15"),
  }).toStrictEqual({ "bg-destructive/10": true, "bg-destructive/15": false });
});
