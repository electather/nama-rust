import { create } from "@bufbuild/protobuf";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RequestSchema, RequestState } from "@/gen/nama/v1/request_pb";
import { RequestPill } from "@/shared/components/request-state/request-pill";

afterEach(cleanup);

test("a Failed Request pill off Popover is red on the 15% tint", () => {
  const request = create(RequestSchema, { state: RequestState.FAILED });
  render(<RequestPill standing={{ kind: "stalled", request, can: "nothing" }} />);
  const pill = screen.getByText("Failed");
  expect({
    "bg-destructive/10": pill.classList.contains("bg-destructive/10"),
    "bg-destructive/15": pill.classList.contains("bg-destructive/15"),
  }).toStrictEqual({ "bg-destructive/10": false, "bg-destructive/15": true });
});
