import { useRef, useState } from "react";

import type { Refused } from "@/features/settings/lib/refused-words";

/** `saved` with the changes still being `sent` over it, the newest last. */
function shown<Values extends object>(saved: Values, sent: Partial<Values>[]): Values {
  const values = { ...saved };
  for (const changes of sent) {
    Object.assign(values, changes);
  }
  return values;
}

/**
 * A pane's `saved` settings, each change saved as it's made: `save` gets the values and which of
 * them changed, and rejects if the Server refuses. A change shows at once, and goes back to what's
 * saved if it's refused. One that's saved says nothing.
 */
export function useAutoSave<Values extends object>(
  saved: Values,
  save: (values: Values, changed: (keyof Values & string)[]) => Promise<unknown>,
) {
  // The changes on their way to the Server, shown over what's saved until it answers.
  const [sending, setSending] = useState<Partial<Values>[]>([]);
  const [refused, setRefused] = useState<Refused>();
  // One save at a time, in the order the changes were made, so the last one made is the one kept.
  const last = useRef(Promise.resolve());
  const values = shown(saved, sending);

  /** Saves `changes` once the saves before them are `done`. It never rejects: a refusal is kept. */
  async function send(done: Promise<void>, changes: Partial<Values>) {
    const keys = Object.keys(changes).filter((key): key is keyof Values & string => key in saved);
    await done;
    try {
      await save({ ...values, ...changes }, keys);
    } catch (error) {
      // ponytail: a pane left before the Server answers never says its refusal. Say it from the
      // app's shell once something outside a pane can.
      setRefused({ failure: error, keys });
    }
    setSending((now) => now.filter((each) => each !== changes));
  }

  return {
    values,
    /** The change the Server refused, until it's dismissed or the next one is made. */
    refused,
    /** Shows `changes` at once, and saves them. */
    edit: (changes: Partial<Values>) => {
      setSending((now) => [...now, changes]);
      setRefused(undefined);
      last.current = send(last.current, changes);
    },
    dismiss: () => {
      setRefused(undefined);
    },
  };
}
