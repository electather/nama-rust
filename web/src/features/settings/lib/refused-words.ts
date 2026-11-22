import { andList } from "@/features/settings/lib/and-list";
import { type FormErrors, refusal } from "@/shared/lib/forms";

/** A change the Server refused: what it refused it with, and the settings the change was to. */
export interface Refused {
  failure: unknown;
  keys: string[];
}

/**
 * What a pane says of the change the Server `refused`, naming its settings by `labels`: in its
 * toast, or under a field where `byReason` puts it. Nothing while none was refused.
 */
export function refusedWords(
  refused: Refused | undefined,
  labels: Record<string, { label: string }>,
  byReason?: Parameters<typeof refusal>[3],
): FormErrors {
  if (refused === undefined) {
    return {};
  }
  const names = refused.keys.map((key) => labels[key]?.label ?? key);
  return refusal(refused.failure, `save ${andList(names)}`, {}, byReason);
}
