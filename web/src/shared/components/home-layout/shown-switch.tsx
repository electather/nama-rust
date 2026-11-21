import { Switch } from "@/shared/ui/switch";

/** Whether Home shows the row called `label`. */
export function ShownSwitch({
  label,
  shown,
  onShownChange,
}: {
  label: string;
  shown: boolean;
  onShownChange: (shown: boolean) => void;
}) {
  return <Switch aria-label={`Show ${label}`} checked={shown} onCheckedChange={onShownChange} />;
}
