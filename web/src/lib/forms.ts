import { Code, ConnectError } from "@connectrpc/connect";

/** Why setting up or signing in was refused: next to one of the fields, or for the `form` as a whole. */
export interface FormErrors {
  form?: string;
  setupCode?: string;
  name?: string;
  password?: string;
  deviceName?: string;
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
