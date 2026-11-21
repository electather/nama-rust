import { StoreOptions } from "@/features/settings/components/store-options";
import { useSwitchImageStore } from "@/features/settings/data/use-switch-image-store";
import { switchRefusal } from "@/features/settings/lib/image-store";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { FormSelect } from "@/shared/components/form-field/form-select";
import { formText, onSubmitted } from "@/shared/lib/forms";

/** The picker of stores, `current` chosen, and the button that switches to the one picked. */
export function SwitchForm({
  current,
  stores,
  running,
  nameOf,
}: {
  current: string;
  stores: PluginSummary[];
  running: boolean;
  nameOf: (pluginId: string) => string;
}) {
  const switchStore = useSwitchImageStore();
  const failed = switchStore.error === null ? undefined : switchRefusal(switchStore.error, nameOf);

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={onSubmitted((typed) => {
        // A disabled option isn't submitted, so the store in use, when disabled, sends no pluginId.
        const pluginId = typed.has("pluginId") ? formText(typed, "pluginId") : current;
        switchStore.mutate({ pluginId });
      })}
    >
      <FormSelect
        row
        label="Keep images on"
        name="pluginId"
        defaultValue={current}
        // Local disk is the empty value, which a required select would refuse.
        required={false}
        hint="An enabled Blob store Plugin, such as S3, can keep them in place of local disk. Every image is copied across before the switch, and the old store keeps its copies."
      >
        <StoreOptions current={current} stores={stores} nameOf={nameOf} />
      </FormSelect>
      <FormEnd
        action={switchStore.isPending ? "Checking the store…" : "Switch image store"}
        variant="ghost"
        pending={switchStore.isPending || running}
        done={undefined}
        refusal={failed}
      />
    </form>
  );
}
