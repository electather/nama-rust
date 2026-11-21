import { viewerWords } from "@/features/admin-now-playing/lib/play-words";
import { useUsers } from "@/features/admin-users/data/use-users";
import type { Playback } from "@/gen/nama/v1/event_pb";
import { useDevices } from "@/shared/hooks/use-devices";

/** Who plays `playback`, on which Device, such as "Ada on Living room TV". */
export function useViewer({ userId, deviceId }: Playback) {
  const user = useUsers().find(({ id }) => id === userId);
  const { data } = useDevices({ userId }, true);
  return viewerWords(
    user,
    data?.devices.find(({ id }) => id === deviceId),
  );
}
