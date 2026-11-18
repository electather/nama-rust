import { cn } from "cn";
import type { ChangeEvent } from "react";

import { buttonVariants } from "@/shared/ui/button";

/** A button that picks a file of a type in `accept` to upload as the image `name`. */
export function UploadButton({
  name,
  accept,
  pending,
  onPicked,
}: {
  name: string;
  accept: string;
  pending: boolean;
  onPicked: (file: File) => void;
}) {
  function picked(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    // Cleared, so picking the same file again after a refusal still uploads it.
    event.target.value = "";
    if (file) {
      onPicked(file);
    }
  }
  return (
    <label
      className={cn(
        buttonVariants({ variant: "ghost" }),
        "cursor-pointer has-focus-visible:ring-3 has-focus-visible:ring-ring/50",
        pending && "pointer-events-none bg-muted",
      )}
    >
      Upload
      <input
        type="file"
        accept={accept}
        aria-label={`Upload a ${name}`}
        disabled={pending}
        onChange={picked}
        className="sr-only"
      />
    </label>
  );
}
