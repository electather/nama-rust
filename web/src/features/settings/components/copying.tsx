import type { ImageStoreSwitch } from "@/gen/nama/v1/server_pb";
import { Progress } from "@/shared/ui/progress";

/** The running copy to the store shown as `to`: a bar, and how many images it has copied. */
export function Copying({
  switching: { copied, total },
  to,
}: {
  switching: ImageStoreSwitch;
  to: string;
}) {
  // The Server lists the images before it copies any, and has none to count until then.
  const listing = total === 0;
  return (
    <div className="flex flex-col gap-1.5">
      <Progress value={listing ? 0 : (copied / total) * 100} aria-label="Images copied" />
      <p className="text-sm text-muted-foreground">
        {listing
          ? `Listing the images to copy to ${to}…`
          : `Copied ${copied} of ${total} images to ${to}…`}
      </p>
    </div>
  );
}
