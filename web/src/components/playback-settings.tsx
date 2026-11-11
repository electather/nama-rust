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
import {
  HardwareAcceleration,
  PlaybackSettingsSchema,
  ServerService,
} from "@/gen/nama/v1/server_pb";
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

/** What ffmpeg and ffprobe the Server found, and the hardware acceleration that passed its test. */
export function playbackStatusQuery(transport: Transport) {
  return {
    ...createQueryOptions(ServerService.method.getPlaybackStatus, {}, { transport }),
    staleTime: "static" as const,
  };
}

const hardwareNames = new Map<HardwareAcceleration, string>([
  [HardwareAcceleration.UNSPECIFIED, "Automatic"],
  [HardwareAcceleration.OFF, "Off"],
  [HardwareAcceleration.VAAPI, "VAAPI"],
  [HardwareAcceleration.QSV, "Intel QSV"],
  [HardwareAcceleration.NVENC, "NVIDIA NVENC"],
  [HardwareAcceleration.VIDEOTOOLBOX, "Apple VideoToolbox"],
]);

/** A hardware acceleration API's name in plain words, as the Server's own messages say it. */
export function hardwareName(api: HardwareAcceleration) {
  return hardwareNames.get(api) ?? `API ${api}`;
}

/**
 * The form an Admin sets how the Server plays Versions it changes with: the folder Remuxes and
 * Transcodes write their segment files to, what Transcodes run on, the most it runs at once, and
 * the default Playback Limit for remote playback. One form, as saving replaces every Playback
 * setting.
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
      <HardwareField saved={settings.hardwareAcceleration} />
      <FormField
        label="Transcodes at once"
        name="maxTranscodes"
        type="number"
        min={1}
        required={false}
        defaultValue={settings.maxTranscodes}
        hint="The most the Server runs at once. Remuxes don’t count. While that many run, a Device that can play a Remux gets one instead, and one that can’t is told the Server is busy. Leave it empty for no limit while Transcodes use hardware acceleration, or 2 in software."
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

/** The choice of what Transcodes run on: Automatic, Off, or an API that passed its test. */
function HardwareField({ saved }: { saved: HardwareAcceleration }) {
  const { hardware } = useSuspenseQuery(ServerService.method.getPlaybackStatus, {}).data;
  const choices = [
    HardwareAcceleration.UNSPECIFIED,
    HardwareAcceleration.OFF,
    // An API that passed on two GPUs is one choice: Automatic's order picks the GPU.
    ...new Set(hardware.map(({ api }) => api)),
  ];
  // A saved API that didn't pass its test this time stays offered, so saving doesn't change it.
  if (!choices.includes(saved)) choices.push(saved);
  return (
    <FormSelect
      label="Hardware acceleration"
      name="hardwareAcceleration"
      defaultValue={saved}
      hint="What Transcodes decode, scale and encode video on. Automatic picks the one marked above, and Off runs them in software. One that didn’t pass its test when the Server started leaves them in software. A change applies from the next Transcode."
    >
      {choices.map((api) => (
        <option key={api} value={api}>
          {hardwareName(api)}
        </option>
      ))}
    </FormSelect>
  );
}

/** The Playback settings the form holds `typed`. */
function typedSettings(typed: FormData) {
  const maxTranscodes = formText(typed, "maxTranscodes");
  return {
    segmentFolder: formText(typed, "segmentFolder"),
    // Empty leaves the cap unset, for the Server's default.
    ...(maxTranscodes !== "" && { maxTranscodes: Number(maxTranscodes) }),
    hardwareAcceleration: typedHardware(formText(typed, "hardwareAcceleration")),
    remotePlaybackLimit: limitPicked(formText(typed, "remotePlaybackLimit")),
  };
}

/** The hardware acceleration the option `value` stands for. */
function typedHardware(value: string): HardwareAcceleration {
  return (
    Object.values(HardwareAcceleration).find((api) => String(api) === value) ??
    HardwareAcceleration.UNSPECIFIED
  );
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
