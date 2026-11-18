import { Share } from "lucide-react";

import { Button } from "@/shared/ui/button";

async function share(url: string) {
  try {
    await navigator.share({ url });
  } catch {
    // Dismissing the share sheet rejects, which is no failure.
  }
}

/** Opens the system share sheet with `url`. */
export function ShareLinkButton({ url }: { url: string }) {
  return (
    <Button
      variant="ghost"
      onClick={() => {
        void share(url);
      }}
    >
      <Share aria-hidden className="size-4" />
      Share…
    </Button>
  );
}
