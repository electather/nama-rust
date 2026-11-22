import { cn } from "cn";
import { buttonVariants } from "nama-ui/components/button";

import { handPicked } from "@/shared/lib/picked-file";

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
        onChange={(event) => {
          handPicked(event, onPicked);
        }}
        className="sr-only"
      />
    </label>
  );
}
