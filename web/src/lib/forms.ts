import { Code, ConnectError } from "@connectrpc/connect";
import type { SubmitEvent } from "react";

/** Why a form was refused: next to one of its fields, or for the `form` as a whole. */
export interface FormErrors {
  form?: string;
  setupCode?: string;
  name?: string;
  password?: string;
  currentPassword?: string;
  newPassword?: string;
}

/**
 * A native form's `onSubmit` that hands `handle` what was typed, and the form itself. It stops the
 * browser's own submit, which would clear every field after a refusal.
 */
export function onSubmitted(handle: (typed: FormData, form: HTMLFormElement) => void) {
  return (event: SubmitEvent<HTMLFormElement>) => {
    event.preventDefault();
    handle(new FormData(event.currentTarget), event.currentTarget);
  };
}

/** The text `form` holds for the input named `name`, or `""` without one. */
export function formText(form: FormData, name: string) {
  const value = form.get(name);
  return typeof value === "string" ? value : "";
}

/**
 * The messages for a form refused with `error`: the ones `byCode` gives for its Connect code, or
 * else a message for the whole form saying the `attempt` (such as "sign in") failed.
 */
export function refusal(
  error: unknown,
  attempt: string,
  byCode: Partial<Record<Code, FormErrors>>,
): FormErrors {
  const { code } = ConnectError.from(error);
  if (code === Code.ResourceExhausted) {
    return { form: "There have been too many tries. Try again in a moment." };
  }
  return (
    byCode[code] ?? {
      form: `Couldn’t ${attempt}. Check that the Server is running, then try again.`,
    }
  );
}

/** The Server's own message for `error`, as a sentence. */
export function serverMessage(error: unknown) {
  const { rawMessage } = ConnectError.from(error);
  return `${rawMessage.charAt(0).toUpperCase()}${rawMessage.slice(1)}.`;
}

// ponytail: the Server names the field only in its message ("a User's name needs…"). Read it
// from Connect error details instead if the Server starts sending them.
/** Which of a username or password field an `invalid_argument` refusal is about. */
export function invalidField(error: unknown): "name" | "password" {
  const message = String(error);
  if (message.includes("User's name")) return "name";
  return "password";
}
