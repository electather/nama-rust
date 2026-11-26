import { useState } from "react";

import { useSetSwitchPin } from "@/features/settings/data/use-set-switch-pin";
import { switchPinRefused } from "@/features/settings/lib/switch-pin";
import { useClearSwitchPin } from "@/shared/hooks/use-clear-switch-pin";
import { formText } from "@/shared/lib/forms";

/** Setting or clearing the signed-in User's own Switch PIN from the Sign-in pane. */
export function useSwitchPinChange() {
  const setPin = useSetSwitchPin();
  const clearPin = useClearSwitchPin();
  const [shown, setShown] = useState<{
    done?: string;
    errors: ReturnType<typeof switchPinRefused>;
  }>({ errors: {} });

  async function change(form: HTMLFormElement, done: string, call: () => Promise<unknown>) {
    setShown({ errors: {} });
    try {
      await call();
    } catch (error) {
      setShown({ errors: switchPinRefused(error) });
      return;
    }
    form.reset();
    setShown({ done, errors: {} });
  }

  return {
    ...shown,
    pending: setPin.isPending || clearPin.isPending,
    set: async (typed: FormData, form: HTMLFormElement) => {
      const switchPin = formText(typed, "switchPin");
      await change(form, "Switch PIN set", async () => {
        await setPin.mutateAsync({ switchPin });
      });
    },
    clear: async (form: HTMLFormElement) => {
      await change(form, "Switch PIN cleared", async () => {
        await clearPin.mutateAsync({});
      });
    },
  };
}
