import { useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import type { Opening } from "@/features/action-bar/lib/menu";

function openingFor(search: string | undefined): Opening | undefined {
  return search === undefined ? undefined : { frame: "root", query: search };
}

/**
 * What the action bar is open on, and a setter. `search`, from the URL, opens it with that typed,
 * and leaves the URL at once, so Back and a later visit don't open it again.
 */
export function useActionBarOpening(search: string | undefined) {
  const navigate = useNavigate();
  const [opening, setOpening] = useState(() => openingFor(search));
  const [searched, setSearched] = useState(search);
  if (search !== searched) {
    setSearched(search);
    setOpening(openingFor(search) ?? opening);
  }
  useEffect(() => {
    if (search !== undefined) {
      void navigate({
        to: ".",
        search: (before) => ({ ...before, search: undefined }),
        replace: true,
      });
    }
  }, [search, navigate]);
  return [opening, setOpening] as const;
}
