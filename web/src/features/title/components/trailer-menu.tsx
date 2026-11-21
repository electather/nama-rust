import { Share } from "lucide-react";

import { CardMoreActions } from "@/features/title/components/card-more-actions";
import type { Trailer } from "@/gen/nama/v1/request_pb";
import { useShare } from "@/shared/hooks/use-share";
import { DropdownMenuItem } from "@/shared/ui/dropdown-menu";

/** A Trailer card's "…": Share its link, or copy it where the system can't share. */
export function TrailerMenu({ trailer }: { trailer: Trailer }) {
  const share = useShare();
  return (
    <CardMoreActions label={`More for ${trailer.name}`}>
      <DropdownMenuItem
        onClick={() => {
          void share({ url: trailer.url, title: trailer.name });
        }}
      >
        <Share aria-hidden className="size-4" />
        Share
      </DropdownMenuItem>
    </CardMoreActions>
  );
}
