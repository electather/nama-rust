import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import { fireEvent, screen, within } from "@testing-library/react";
import { expect } from "vite-plus/test";

import { RefusalReason, RefusalSchema } from "@/gen/nama/v1/refusal_pb";

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
  const field = screen.getByLabelText(label, { selector: "input, textarea, select" });
  expect(field.getAttribute("aria-invalid")).toBe("true");
  expect(field.getAttribute("aria-describedby")).toContain(error.id);
}

/** The Server's refusal of a sensitive change from a Device without Recent Authentication. */
export function recentAuthenticationRequired() {
  return refused(Code.FailedPrecondition, "sign in again to do this", {
    reason: RefusalReason.RECENT_AUTHENTICATION_REQUIRED,
  });
}

/** Waits for the "Confirm it’s you" dialog, and confirms with `password`. */
export async function confirmItsYou(password = "correct horse") {
  const dialog = await screen.findByRole("alertdialog", { name: "Confirm it’s you" });
  fireEvent.change(within(dialog).getByLabelText("Password"), { target: { value: password } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Confirm" }));
  return dialog;
}
