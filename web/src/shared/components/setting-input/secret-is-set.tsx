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
    <div className="flex items-start justify-between gap-4">
      <div>
        <p className="text-sm font-medium">{setting.label}</p>
        <p className="text-sm text-pretty text-muted-foreground empty:hidden">
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
