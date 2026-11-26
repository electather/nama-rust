import { useSearch } from "@tanstack/react-router";

import { useUsers } from "@/shared/hooks/use-users";

/**
 * The User `?user=` opens in the Users section: none for an id no User has, such as an address from
 * before they were deleted, so the list shows then.
 */
export function useOpenUser() {
  const asked = useSearch({ strict: false, select: ({ user }) => user });
  return useUsers().find((user) => Number(user.id) === asked);
}
