import { Search } from "lucide-react";
import type { RefObject } from "react";

import { FoundSubtitles } from "@/features/player/components/found-subtitles";
import { SettingsRow } from "@/features/player/components/settings-row";
import { useDownloadSubtitle } from "@/features/player/data/use-download-subtitle";
import { useSearchSubtitles } from "@/features/player/data/use-search-subtitles";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import type { FoundSubtitle } from "@/gen/nama/v1/playback_pb";
import { serverMessage } from "@/shared/lib/forms";

/**
 * "Search for subtitles", then what it found. Picking one downloads it and shows its new Track; a
 * failure is said quietly, leaving the subtitle playing as it was.
 */
export function SubtitleSearch({
  planned,
  videoRef,
}: {
  planned: ReturnType<typeof usePlaybackPlan>;
  videoRef: RefObject<HTMLVideoElement | null>;
}) {
  const { versionId } = planned;
  const search = useSearchSubtitles(versionId);
  const download = useDownloadSubtitle();
  const pick = (subtitle: FoundSubtitle) => {
    const onSuccess = () => {
      if (videoRef.current) {
        planned.chooseDownloaded(videoRef.current, subtitle);
      }
    };
    download.mutate({ versionId, subtitle }, { onSuccess });
  };
  return (
    <>
      <hr className="mx-2.5 my-1 border-border" />
      <SettingsRow
        Icon={Search}
        label={search.isPending ? "Searching…" : "Search for subtitles"}
        onClick={() => {
          search.mutate();
        }}
      />
      <FoundSubtitles
        found={search.data}
        busy={download.isPending}
        picked={download.variables?.subtitle}
        pick={pick}
      />
      <output className="block px-2.5 py-2 text-muted-foreground empty:hidden">
        {statusText(search.error ?? download.error, search.data)}
      </output>
    </>
  );
}

/** What the search says quietly: why it or the download failed, or that it found nothing. */
function statusText(failed: Error | null, found: FoundSubtitle[] | undefined) {
  if (failed) {
    return serverMessage(failed);
  }
  return found?.length === 0 ? "Found no subtitles." : "";
}
