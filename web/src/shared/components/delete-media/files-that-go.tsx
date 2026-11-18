import { Cancelled } from "@/shared/components/delete-media/cancelled";
import { FilesList } from "@/shared/components/delete-media/files-list";
import { RefusedFor } from "@/shared/components/delete-media/refused-for";
import { TakenAlong } from "@/shared/components/delete-media/taken-along";
import { Watching } from "@/shared/components/delete-media/watching";
import { usePreviewDeleteMedia } from "@/shared/hooks/use-preview-delete-media";
import type { Deletable } from "@/shared/lib/deletable";
import { refusal } from "@/shared/lib/forms";

/**
 * The files deleting `chosen` of `deletable` takes, the space that frees, the other Episodes
 * those files hold, the Requests it cancels and the read-only folder that refuses it if a scan
 * found one since the page loaded, as the Server previews it.
 */
export function FilesThatGo({ deletable, chosen }: { deletable: Deletable; chosen: string }) {
  const { data, error } = usePreviewDeleteMedia(deletable, chosen);
  if (error) {
    return <p className="text-sm text-destructive">{refusal(error, "find its files", {}).form}</p>;
  }
  if (!data) {
    return <p className="text-sm text-muted-foreground">Finding its files…</p>;
  }
  return (
    <div className="flex flex-col gap-3 text-sm">
      <FilesList files={data.files} bytesFreed={data.bytesFreed} />
      <TakenAlong episodes={data.episodesTakenAlong} />
      <Cancelled requests={data.requestsCancelled} />
      <Watching playing={data.playing} />
      <RefusedFor folder={data.readOnlyFolder} />
    </div>
  );
}
