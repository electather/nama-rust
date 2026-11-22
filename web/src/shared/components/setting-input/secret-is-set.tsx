import { Pencil, X } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { useState } from "react";

import type { Setting } from "@/gen/nama/v1/plugin_pb";
import { ForgetSecret } from "@/shared/components/setting-input/forget-secret";
import { SettingRow } from "@/shared/components/setting-row";
import type { SecretAction } from "@/shared/lib/setting-values";

/**
 * A secret that is set, in the field it would be typed in: masked, as its value never comes back,
 * with a pencil that replaces it and, if `forgettable`, the X that forgets it once that's confirmed in its place.
 */
export function SecretIsSet({
  setting,
  forgettable,
  onPress,
}: {
  setting: Setting;
  forgettable: boolean;
  onPress: (action: SecretAction) => void;
}) {
  const [confirming, setConfirming] = useState(false);
  const masked = (
    <span className="flex h-11 min-w-0 flex-1 items-center rounded-lg border border-input bg-input/30 ps-2.5 text-sm tracking-widest text-muted-foreground md:h-8">
      <span aria-hidden>••••••••••••</span>
      <span className="sr-only">Set</span>
    </span>
  );
  const forget = forgettable && (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      aria-label={`Forget ${setting.label}`}
      onClick={() => {
        setConfirming(true);
      }}
    >
      <X aria-hidden strokeWidth={1.5} />
    </Button>
  );
  const set = (
    <span className="flex w-full items-center gap-1 md:w-60">
      {masked}
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={`Replace ${setting.label}`}
        onClick={() => {
          onPress("Replace");
        }}
      >
        <Pencil aria-hidden strokeWidth={1.5} />
      </Button>
      {forget}
    </span>
  );
  return (
    <SettingRow wide label={setting.label} hint={setting.description}>
      {() =>
        confirming ? (
          <ForgetSecret
            label={setting.label}
            onForget={() => {
              onPress("Forget");
            }}
            onCancel={() => {
              setConfirming(false);
            }}
          />
        ) : (
          set
        )
      }
    </SettingRow>
  );
}
