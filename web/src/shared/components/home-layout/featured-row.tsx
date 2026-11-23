import { Pin, Sparkles } from "lucide-react";

import { RowLabel } from "@/shared/components/home-layout/row-label";
import { ShownSwitch } from "@/shared/components/home-layout/shown-switch";
import { IconTile } from "@/shared/components/icon-tile";
import { homeLabels } from "@/shared/lib/home-layout";

/** Featured, which is always first, so it's pinned where the other rows have a grip, and only shows or hides. */
export function FeaturedRow({
  shown,
  onShownChange,
}: {
  shown: boolean;
  onShownChange: (shown: boolean) => void;
}) {
  const { label, hint } = homeLabels.featured;
  return (
    <div data-setting="featured" data-hidden={!shown || undefined} className="group flex gap-x-3">
      <span
        aria-hidden
        className="flex shrink-0 items-center gap-2 py-1.5 transition-opacity group-data-hidden:opacity-45 motion-reduce:transition-none"
      >
        <Pin className="size-3.5 text-muted-foreground/60" />
        <IconTile Icon={Sparkles} />
      </span>
      <div className="flex min-w-0 flex-1 items-center gap-3 border-b py-1.5">
        <RowLabel name={{ label, from: hint }} />
        <ShownSwitch label={label} shown={shown} onShownChange={onShownChange} />
      </div>
    </div>
  );
}
