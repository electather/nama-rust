import { useServerName } from "@/shared/hooks/use-server-name";

/** The setup page's heading, naming the Server once its name has loaded. */
export function useSetupHeading() {
  const serverName = useServerName();
  return serverName === undefined ? "Set up this Server" : `Set up ${serverName}`;
}
