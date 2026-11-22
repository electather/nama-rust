import { type Download, DownloadStatus } from "@/gen/nama/v1/request_pb";

/** Why any of a Request's `downloads` stopped, in its Fulfiller's words. */
export function StoppedDownloads({ downloads }: { downloads: Download[] }) {
  const stopped = downloads.filter(({ status }) => status === DownloadStatus.NEEDS_ATTENTION);
  return stopped.map((download) => (
    <p key={download.title} className="text-xs text-pretty text-muted-foreground">
      {download.title} stopped, and whoever runs the Server needs to look at it.{" "}
      {download.attentionReason}
    </p>
  ));
}
