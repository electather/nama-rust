import { create } from "@bufbuild/protobuf";
import type { ConnectError, Transport } from "@connectrpc/connect";
import {
  createConnectQueryKey,
  createQueryOptions,
  useMutation,
  useSuspenseQuery,
} from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { FormEnd, FormField, FormSelect } from "@/components/form-field";
import { belowOriginal } from "@/components/playback-limit-menu";
import { PlaybackLimit } from "@/gen/nama/v1/playback_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { PlaybackSettingsSchema, ServerService } from "@/gen/nama/v1/server_pb";
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

/**
 * The form an Admin sets how the Server plays Versions it changes with: the folder Remuxes and
 * Transcodes write their segment files to, the most Transcodes it runs at once, and the default
 * Playback Limit for remote playback. One form, as saving replaces every Playback setting.
 */
export function PlaybackSettingsForm() {
  const queryClient = useQueryClient();
  const settings =
    useSuspenseQuery(ServerService.method.getPlaybackSettings, {}).data.settings ??
    create(PlaybackSettingsSchema);
  const update = useMutation(ServerService.method.updatePlaybackSettings, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: playbackSettingsKey }),
  });
  const errors = playbackSettingsErrors(update.error);

  return (
    <form
      className="mt-10 flex max-w-md flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        update.mutate({ settings: typedSettings(typed) });
      })}
    >
      <FormField
        label="Segment folder"
        name="segmentFolder"
        autoComplete="off"
        spellCheck={false}
        placeholder="/dev/shm/nama"
        required={false}
        defaultValue={settings.segmentFolder}
        hint="Where a Remux or Transcode writes the files it plays from, such as a tmpfs, until a minute after they’re last played. Leave it empty for the transcodes folder in the Server’s data folder. A change applies from the next one."
        error={errors.segmentFolder}
      />
      <FormField
        label="Transcodes at once"
        name="maxTranscodes"
        type="number"
        min={1}
        defaultValue={settings.maxTranscodes}
        hint="The most the Server runs at once. Remuxes don’t count. While that many run, a Device that can play a Remux gets one instead, and one that can’t is told the Server is busy."
        error={errors.maxTranscodes}
      />
      <RemoteLimitField saved={settings.remotePlaybackLimit} />
      <FormEnd
        action={update.isPending ? "Saving playback settings…" : "Save playback settings"}
        pending={update.isPending}
        done={update.isSuccess ? "Saved." : undefined}
        refusal={errors.form}
      />
    </form>
  );
}

/** The Playback settings the form holds `typed`. */
function typedSettings(typed: FormData) {
  return {
    segmentFolder: formText(typed, "segmentFolder"),
    maxTranscodes: Number(formText(typed, "maxTranscodes")),
    remotePlaybackLimit: limitPicked(formText(typed, "remotePlaybackLimit")),
  };
}

/** The Playback Limits an Admin can pick from, as the player's menu names them. */
const limits = [
  { limit: PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL, name: "Original" },
  ...belowOriginal,
];

/** The field for the default Playback Limit for remote playback, showing the `saved` one. */
function RemoteLimitField({ saved }: { saved: PlaybackLimit }) {
  return (
    <FormSelect
      label="Default Playback Limit for remote playback"
      name="remotePlaybackLimit"
      defaultValue={saved}
      hint="The most a Device outside the local network is sent until its User picks a Playback Limit in the player. Original sends each Version as it is."
    >
      {limits.map(({ limit, name }) => (
        <option key={limit} value={limit}>
          {name}
        </option>
      ))}
    </FormSelect>
  );
}

/** The Playback Limit an `<option>`'s `value` names, or else Original. */
function limitPicked(value: string) {
  const picked = limits.find(({ limit }) => String(limit) === value);
  return picked?.limit ?? PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL;
}

/** Why the Server refused the Playback settings, under their field, or `{}` while it hasn't. */
function playbackSettingsErrors(error: ConnectError | null) {
  if (error === null) return {};
  return refusal(
    error,
    "save the playback settings",
    {},
    {
      [RefusalReason.FOLDER_NOT_ABSOLUTE]: { segmentFolder: serverMessage(error) },
      // The only field the Server refuses as missing here, a cap of 0.
      [RefusalReason.FIELD_REQUIRED]: { maxTranscodes: serverMessage(error) },
    },
  );
}
