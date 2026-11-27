import { useSettingsAddress } from "@/shared/hooks/use-settings-address";
import { useUsers } from "@/shared/hooks/use-users";

/**
 * The User `?user=` opens in the Users section: none for an id no User has, such as an address from
 * before they were deleted, so the list shows then.
 */
export function useOpenUser() {
  const { detail } = useSettingsAddress();
  return useUsers().find((user) => detail?.kind === "user" && Number(user.id) === detail.id);
}
