import { useState } from "react";

import type { Setting } from "@/gen/nama/v1/plugin_pb";
import { SecretIsSet } from "@/shared/components/setting-input/secret-is-set";
import { TypedInput } from "@/shared/components/setting-input/typed-input";
import { type SecretAction, forgottenField } from "@/shared/lib/setting-values";

/** A secret that is set: said to be set, until a button swaps in an input to type in its place. */
export function SetSecret({
  setting,
  error,
  forgettable,
}: {
  setting: Setting;
  error: string | undefined;
  forgettable: boolean;
}) {
  const [pressed, setPressed] = useState<SecretAction>();
  // A refusal naming the secret needs its input to show under.
  if (pressed === undefined && error === undefined) {
    return <SecretIsSet setting={setting} forgettable={forgettable} onPress={setPressed} />;
  }
  const forgetting = pressed === "Forget";
  return (
    <>
      {forgetting && <input type="hidden" name={forgottenField} value={setting.key} />}
      <TypedInput
        {...{ setting, error, forgetting }}
        required={false}
        focused={pressed !== undefined}
      />
    </>
  );
}
