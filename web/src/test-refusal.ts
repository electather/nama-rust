import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { type Code, ConnectError } from "@connectrpc/connect";
import { screen } from "@testing-library/react";
import { expect } from "vite-plus/test";

import { RefusalSchema } from "@/gen/nama/v1/refusal_pb";

/** A Connect error as the Server refuses with one: `message`, plus the `refusal` detail. */
export function refused(
  code: Code,
  message: string,
  refusal: MessageInitShape<typeof RefusalSchema>,
) {
  return new ConnectError(message, code, undefined, [
    { desc: RefusalSchema, value: create(RefusalSchema, refusal) },
  ]);
}

/** Expects `message` to show under the field labelled `label`, marking it invalid. */
export async function expectUnderField(label: string, message: string) {
  const error = await screen.findByText(message);
  // A label can also name a list, such as a Library page's "Folders".
  const field = screen.getByLabelText(label, { selector: "input, textarea" });
  expect(field.getAttribute("aria-invalid")).toBe("true");
  expect(field.getAttribute("aria-describedby")).toContain(error.id);
}
