import { useSettingsAddress } from "@/shared/hooks/use-settings-address";

/**
 * The search a link to a title's page takes: at the Hub, the Server whose Settings `?manage=`
 * picked, so the page opens on it (ADR 0105).
 */
export function useTitleLinkSearch() {
  const { server } = useSettingsAddress();
  // Set even when undefined, so `retainSearchParams` doesn't keep another Server's `?server=`.
  return { server };
}
