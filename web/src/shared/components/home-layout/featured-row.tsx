import { ShownSwitch } from "@/shared/components/home-layout/shown-switch";
import { homeLabels } from "@/shared/lib/home-layout";

/** Featured, which is always at the top, so it only shows or hides. */
export function FeaturedRow({
  shown,
  onShownChange,
}: {
  shown: boolean;
  onShownChange: (shown: boolean) => void;
}) {
  const { label, hint } = homeLabels.featured;
  return (
    <div data-setting="featured" className="-mx-3 flex items-center gap-3 border-b p-3 ps-12">
      <div className="min-w-0 flex-1">
        <p>{label}</p>
        <p className="text-sm text-muted-foreground">{hint}</p>
      </div>
      <ShownSwitch label={label} shown={shown} onShownChange={onShownChange} />
    </div>
  );
}
