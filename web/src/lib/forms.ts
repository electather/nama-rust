import { create } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import type { SubmitEvent } from "react";

import { RefusalReason, RefusalSchema } from "@/gen/nama/v1/refusal_pb";

import { permissionLabel } from "./permissions";

/** Why a form was refused: next to one of its fields, or for the `form` as a whole. */
export interface FormErrors {
  form?: string;
  setupCode?: string;
  name?: string;
  password?: string;
  deviceName?: string;
  signInCode?: string;
  newPassword?: string;
  folders?: string;
  serverUrl?: string;
  segmentFolder?: string;
  maxTranscodes?: string;
  keepBehindMinutes?: string;
  hardwareAcceleration?: string;
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
 * The messages for a form refused with `error`: the ones `byReason` gives for its Refusal's
 * reason, which is for a Connect code that means more than one thing, or else the ones `byCode`
 * gives for its code, or else a message for the whole form. A call the User cancelled wasn't
 * refused, so it has none.
 */
export function refusal(
  error: unknown,
  attempt: string,
  byCode: Partial<Record<Code, FormErrors>>,
  byReason: Partial<Record<RefusalReason, FormErrors>> = {},
): FormErrors {
  const { code } = ConnectError.from(error);
  if (code === Code.Canceled) return {};
  // Still missing after "Confirm it’s you", it shares `failed_precondition` with refusals forms
  // explain by code, such as "the only Admin".
  const reasons: Partial<Record<RefusalReason, FormErrors>> = {
    [RefusalReason.RECENT_AUTHENTICATION_REQUIRED]: { form: serverMessage(error) },
    [RefusalReason.PERMISSION_REQUIRED]: { form: permissionRequired(error) },
    ...byReason,
  };
  return reasons[refusalOf(error).reason] ?? byCode[code] ?? { form: unexplained(code, attempt) };
}

/** Which Permission the change refused with `error` needs, by the name the Refusal gives it. */
function permissionRequired(error: unknown) {
  const label = permissionLabel(refusalOf(error).params["permission"] ?? "");
  return label === undefined
    ? "You don’t have the Permission this needs."
    : `This needs the ${label} Permission.`;
}

/** Why a Quality was refused with `error`: each Season of `show` takes the Show's, which it names. */
export function showHasQuality(error: unknown, show: string) {
  const quality = refusalOf(error).params["showQualityName"];
  return `Each Season of ${show} takes its Quality${quality === undefined ? "" : `, ${quality}`}.`;
}

/**
 * The message for a refusal no map explains: too many tries for `resource_exhausted`, or otherwise
 * that the `attempt` (such as "sign in") failed.
 */
function unexplained(code: Code, attempt: string) {
  return code === Code.ResourceExhausted
    ? "There have been too many tries. Try again in a moment."
    : `Couldn’t ${attempt}. Check that the Server is running, then try again.`;
}

/** The Server's own message for `error`, as a sentence. */
export function serverMessage(error: unknown) {
  const { rawMessage } = ConnectError.from(error);
  const sentence = `${rawMessage.charAt(0).toUpperCase()}${rawMessage.slice(1)}`;
  // A Plugin's own message may already end its sentence.
  return /[.!?]$/u.test(sentence) ? sentence : `${sentence}.`;
}

/**
 * `error`'s message next to the field of `fields` its Refusal names, or for the whole form when
 * it names none of them.
 */
export function refusedFieldErrors(error: unknown, fields: (keyof FormErrors)[]): FormErrors {
  const field = fields.find((each) => each === refusedField(error)) ?? "form";
  return { [field]: serverMessage(error) };
}

/** The form field the Refusal on `error` is about, or `""` if it names none. */
function refusedField(error: unknown) {
  // The Server names a proto field path (`new_password`, `folders[2]`); forms key on
  // `newPassword` and `folders`.
  return refusalOf(error)
    .field.replace(/\[\d+\]$/u, "")
    .replaceAll(/_(\w)/gu, (_, letter: string) => letter.toUpperCase());
}

// web/AGENTS.md, "Read refusals by reason".
/** The Refusal `error` carries, or an empty one, with no reason or field, if it carries none. */
export function refusalOf(error: unknown) {
  return ConnectError.from(error).findDetails(RefusalSchema)[0] ?? create(RefusalSchema);
}

/** Why the Server refused the `attempt` to ask for a title, as `byReason` explains it or else by code. */
export function askRefusal(
  error: unknown,
  attempt: string,
  byReason: Parameters<typeof refusal>[3],
) {
  return refusal(
    error,
    attempt,
    { [Code.Unavailable]: { form: "The metadata source didn’t answer. Try again in a moment." } },
    byReason,
  ).form;
}
