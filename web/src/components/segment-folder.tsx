import type { ConnectError, Transport } from "@connectrpc/connect";
import {
  createConnectQueryKey,
  createQueryOptions,
  useMutation,
  useSuspenseQuery,
} from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { FormEnd, FormField } from "@/components/form-field";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { formText, onSubmitted, refusal, serverMessage } from "@/lib/forms";

const playbackSettingsKey = createConnectQueryKey({
  schema: ServerService.method.getPlaybackSettings,
  cardinality: "finite",
});

/** The Playback settings, for the Playback page to load before it shows the form. */
export function playbackSettingsQuery(transport: Transport) {
  return {
    ...createQueryOptions(ServerService.method.getPlaybackSettings, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** The form an Admin sets the folder Remuxes and Transcodes write their segment files to with. */
export function SegmentFolder() {
  const queryClient = useQueryClient();
  const { settings } = useSuspenseQuery(ServerService.method.getPlaybackSettings, {}).data;
  const update = useMutation(ServerService.method.updatePlaybackSettings, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: playbackSettingsKey }),
  });
  const errors = segmentFolderErrors(update.error);

  return (
    <form
      className="mt-10 flex max-w-md flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        update.mutate({ settings: { segmentFolder: formText(typed, "segmentFolder") } });
      })}
    >
      <FormField
        label="Segment folder"
        name="segmentFolder"
        autoComplete="off"
        spellCheck={false}
        placeholder="/dev/shm/nama"
        required={false}
        defaultValue={settings?.segmentFolder}
        hint="Where a Remux or Transcode writes the files it plays from, such as a tmpfs, until a minute after they’re last played. Leave it empty for the transcodes folder in the Server’s data folder. A change applies from the next one."
        error={errors.segmentFolder}
      />
      <FormEnd
        action={update.isPending ? "Saving segment folder…" : "Save segment folder"}
        pending={update.isPending}
        done={update.isSuccess ? "Saved." : undefined}
        refusal={errors.form}
      />
    </form>
  );
}

/** Why the Server refused the segment folder, under its field, or `{}` while it hasn't. */
function segmentFolderErrors(error: ConnectError | null) {
  if (error === null) return {};
  return refusal(
    error,
    "save the segment folder",
    {},
    { [RefusalReason.FOLDER_NOT_ABSOLUTE]: { segmentFolder: serverMessage(error) } },
  );
}
