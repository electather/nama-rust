import { ShownSwitch } from "@/features/admin-home/components/shown-switch";

/** Featured, which is always at the top, so it only shows or hides. */
export function FeaturedRow({
  shown,
  onShownChange,
}: {
  shown: boolean;
  onShownChange: (shown: boolean) => void;
}) {
  return (
    <div className="-mx-3 flex items-center gap-3 border-b p-3 ps-12">
      <div className="min-w-0 flex-1">
        <p>Featured</p>
        <p className="text-sm text-muted-foreground">Always at the top</p>
      </div>
      <ShownSwitch label="Featured" shown={shown} onShownChange={onShownChange} />
    </div>
  );
}
