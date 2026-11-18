import { useLocation, useNavigate, useSearch } from "@tanstack/react-router";

import { Input } from "@/shared/ui/input";

/**
 * Searches the signed-in User's Libraries as they type, on the search page, which keeps the query
 * in its URL. Typing there replaces its history entry, so Back leaves the search.
 */
export function SearchBox() {
  const navigate = useNavigate();
  const onSearchPage = useLocation({ select: ({ pathname }) => pathname === "/search" });
  // Only the search page has a `query`. Read once: after that, only the box changes it.
  const { query } = useSearch({ strict: false });
  return (
    // On phones it takes a row of its own under the links, its edge lined up with theirs.
    <search className="order-last mt-2 w-full px-2.5 sm:order-0 sm:ms-auto sm:mt-0 sm:w-56 sm:px-0">
      <Input
        type="search"
        aria-label="Search your Libraries"
        placeholder="Search"
        autoComplete="off"
        defaultValue={query}
        onChange={(event) => {
          const typed = event.target.value.trim();
          void navigate({
            to: "/search",
            search: typed === "" ? {} : { query: typed },
            replace: onSearchPage,
          });
        }}
      />
    </search>
  );
}
