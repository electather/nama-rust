import type { Setting } from "@/gen/nama/v1/plugin_pb";
import type { SecretAction } from "@/shared/lib/setting-values";
import { Button } from "@/shared/ui/button";

/** Says a secret is set, with the buttons to replace it or, if `forgettable`, forget it. */
export function SecretIsSet({
  setting,
  forgettable,
  onPress,
}: {
  setting: Setting;
  forgettable: boolean;
  onPress: (action: SecretAction) => void;
}) {
  const actions: SecretAction[] = forgettable ? ["Replace", "Forget"] : ["Replace"];
  return (
    // Spaced and ruled as `SettingRow` is, which it sits among.
    <div className="flex items-center justify-between gap-x-5 py-3.5 not-first:border-t">
      <div className="min-w-0 flex-1">
        <p className="text-sm">{setting.label}</p>
        <p className="text-xs text-pretty text-muted-foreground empty:hidden">
          {setting.description}
        </p>
      </div>
      {/* A ghost button is pulled back by its own padding, so its words line up with the inputs. */}
      <div className="-me-4 flex shrink-0 items-center gap-2">
        <span className="text-sm">Set</span>
        {actions.map((action) => (
          <Button
            key={action}
            type="button"
            variant="ghost"
            aria-label={`${action} ${setting.label}`}
            onClick={() => {
              onPress(action);
            }}
          >
            {action}
          </Button>
        ))}
      </div>
    </div>
  );
}
