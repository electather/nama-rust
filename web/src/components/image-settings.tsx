import { create } from "@bufbuild/protobuf";
import type { ConnectError, Transport } from "@connectrpc/connect";
import {
  createConnectQueryKey,
  createQueryOptions,
  useMutation,
  useSuspenseQuery,
} from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useId } from "react";

import { FormEnd, FormField } from "@/components/form-field";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { ImageSettingsSchema, ServerService } from "@/gen/nama/v1/server_pb";
import { formText, onSubmitted, refusal, serverMessage } from "@/lib/forms";

const imageSettingsKey = createConnectQueryKey({
  schema: ServerService.method.getImageSettings,
  cardinality: "finite",
});

/** The image settings, for Settings to load before it shows the form. */
export function imageSettingsQuery(transport: Transport) {
  return {
    ...createQueryOptions(ServerService.method.getImageSettings, {}, { transport }),
    staleTime: "static" as const,
  };
}

/**
 * The form a User who manages settings sets how Posters and Backdrops are resized with: whether
 * they are, and how much disk the resized copies take.
 */
export function ImageSettingsForm() {
  const queryClient = useQueryClient();
  const settings =
    useSuspenseQuery(ServerService.method.getImageSettings, {}).data.settings ??
    create(ImageSettingsSchema, { resizeImages: true });
  const update = useMutation(ServerService.method.updateImageSettings, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: imageSettingsKey }),
  });
  const errors = imageSettingsErrors(update.error);

  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        update.mutate({ settings: typedSettings(typed) });
      })}
    >
      <ResizeField saved={settings.resizeImages} />
      <FormField
        label="Space for resized images"
        name="resizedCacheMib"
        type="number"
        min={1}
        required={false}
        defaultValue={settings.resizedCacheMib}
        hint={`The most disk, in MiB, the resized copies take. Past it, those shown least recently are deleted, and made again when they’re next shown. Leave it empty for 1024\u00A0MiB.`}
        error={errors.resizedCacheMib}
      />
      <FormEnd
        action={update.isPending ? "Saving image settings…" : "Save image settings"}
        variant="ghost"
        pending={update.isPending}
        done={update.isSuccess ? "Saved." : undefined}
        refusal={errors.form}
      />
    </form>
  );
}

/** The checkbox that turns resizing on or off, showing the `saved` choice. */
function ResizeField({ saved }: { saved: boolean }) {
  const hintId = useId();
  return (
    <div className="flex flex-col gap-2">
      <label className="flex items-center gap-3 text-sm font-medium">
        <input
          type="checkbox"
          name="resizeImages"
          defaultChecked={saved}
          aria-describedby={hintId}
          className="size-4 accent-foreground"
        />
        Resize images
      </label>
      <p id={hintId} className="text-sm text-pretty text-muted-foreground">
        Sends each Poster and Backdrop at the size it’s shown, so a phone isn’t sent a picture made
        for a TV. Off, every image is sent at full size and the Server does no image work.
      </p>
    </div>
  );
}

/** The image settings the form holds `typed`. */
function typedSettings(typed: FormData) {
  const mib = formText(typed, "resizedCacheMib");
  return {
    resizeImages: typed.has("resizeImages"),
    // Empty leaves it unset, for the Server's default.
    ...(mib !== "" && { resizedCacheMib: Number(mib) }),
  };
}

/** Why the Server refused the image settings, under their field, or `{}` while it hasn't. */
function imageSettingsErrors(error: ConnectError | null) {
  if (error === null) return {};
  return refusal(
    error,
    "save the image settings",
    {},
    { [RefusalReason.FIELD_REQUIRED]: { resizedCacheMib: serverMessage(error) } },
  );
}
