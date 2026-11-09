import { useQuery as useConnectQuery, useTransport } from "@connectrpc/connect-query";
import { useQuery } from "@tanstack/react-query";
import { type RefObject, useId, useRef, useState } from "react";

import { ConfirmModal } from "@/components/confirm-dialog";
import type { Quality } from "@/gen/nama/v1/plugin_pb";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { signedInQuery } from "@/lib/access";
import { formText } from "@/lib/forms";
import { may } from "@/lib/permissions";

/**
 * The Quality picker for Requesting a Movie, which `open` opens when the signed-in User is
 * `opening` a Request, may choose a Quality, and has more than one to choose from: joining a
 * Request keeps its Quality. `open` says whether it opened. `loading` while the Qualities are
 * read, so a Request isn't sent before the User could pick.
 */
export function useQualityPicker(opening: boolean) {
  const mayChoose = useMayChooseQuality(opening);
  const listed = useConnectQuery(
    RequestService.method.listMovieQualities,
    {},
    { enabled: mayChoose },
  );
  const ref = useRef<HTMLDialogElement>(null);
  const [openings, setOpenings] = useState(0);
  const qualities = toPick(mayChoose, listed.data?.qualities);
  const picks = qualities.length > 0;
  return {
    loading: listed.isLoading,
    hasPopup: picks && ("dialog" as const),
    open: () => {
      if (picks) {
        setOpenings((count) => count + 1);
        ref.current?.showModal();
      }
      return picks;
    },
    dialog: { ref, openings, qualities },
  };
}

/**
 * The `listed` Qualities, if the User may choose and there's more than one, or none. Checks
 * `mayChoose` even with `listed`, since a disabled query still gives what's cached.
 */
function toPick(mayChoose: boolean, listed: Quality[] = []) {
  return mayChoose && listed.length > 1 ? listed : [];
}

/** Whether the signed-in User may choose a Quality when `opening` a Request. */
function useMayChooseQuality(opening: boolean) {
  const you = useQuery({ ...signedInQuery(useTransport()), staleTime: "static" }).data;
  return opening && may(you?.user, Permission.CHOOSE_QUALITY);
}

/**
 * A dialog that asks which of `qualities` to Request `title` at, then hands its id to `onPick`,
 * which rejects if the Server refuses. Nothing while there are no `qualities`. Open it with
 * `showModal()` on `ref`, bumping `openings` first.
 */
export function QualityPicker({
  ref,
  openings,
  title,
  qualities,
  onPick,
  refusal,
}: {
  ref: RefObject<HTMLDialogElement | null>;
  openings: number;
  title: string;
  qualities: Quality[];
  onPick: (quality: bigint) => Promise<unknown>;
  refusal: (error: unknown) => string | undefined;
}) {
  if (qualities.length === 0) return null;
  return (
    <ConfirmModal
      ref={ref}
      openings={openings}
      action="Request"
      title={`Request ${title}`}
      description="Pick the Quality it’s downloaded at."
      onConfirm={(typed) => onPick(BigInt(formText(typed, "quality")))}
      refusal={refusal}
    >
      <fieldset>
        {/* The dialog's description already asks for a Quality, so only a screen reader needs it. */}
        <legend className="sr-only">Quality</legend>
        <ul className="flex flex-col">
          {qualities.map((quality) => (
            <QualityChoice key={String(quality.id)} quality={quality} />
          ))}
        </ul>
      </fieldset>
    </ConfirmModal>
  );
}

/** One Quality as a radio button with its description, picked to start with and marked if it's the default. */
function QualityChoice({ quality }: { quality: Quality }) {
  const descriptionId = useId();
  const described = quality.description !== "";
  return (
    <li className="flex min-h-12 flex-col justify-center border-b py-2 last:border-b-0">
      <label className="flex items-center gap-3 font-medium">
        <input
          type="radio"
          name="quality"
          value={String(quality.id)}
          defaultChecked={quality.isDefault}
          aria-describedby={described ? descriptionId : undefined}
          className="size-4 accent-foreground"
        />
        <span>
          {quality.name}
          {quality.isDefault && (
            <span className="font-normal text-muted-foreground"> (Default)</span>
          )}
        </span>
      </label>
      {described && (
        // Lined up with the label's text, past the radio button and the gap after it.
        <p id={descriptionId} className="ps-7 text-sm text-muted-foreground">
          {quality.description}
        </p>
      )}
    </li>
  );
}
