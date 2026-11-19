import { useNavigate, useSearch } from "@tanstack/react-router";
import { useEffect } from "react";

/**
 * The words `?search=` asks the action bar to open on, which is where a link to the old search page
 * lands (`src/routes/_member/search.tsx`). Read once, it's dropped from the URL, so a reload or
 * Back doesn't open the action bar again.
 */
export function useSearchFromUrl() {
  const words = useSearch({ strict: false, select: ({ search }) => search });
  const navigate = useNavigate();
  useEffect(() => {
    if (words !== undefined) {
      void navigate({ to: ".", search: (rest) => ({ ...rest, search: undefined }), replace: true });
    }
  }, [words, navigate]);
  return words;
}
