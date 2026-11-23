import { type KeyboardEvent, useId, useLayoutEffect, useRef } from "react";

import { SettingsHeader } from "@/features/settings/components/settings-header";
import { SettingsPane } from "@/features/settings/components/settings-pane";
import { SettingsSide } from "@/features/settings/components/settings-side";
import { useCloseSettings } from "@/features/settings/hooks/use-close-settings";
import { type Section, groupsFor, openSection } from "@/features/settings/lib/sections";
import type { User } from "@/gen/nama/v1/sign_in_pb";

/** Where typed text lives; a field of another kind, such as a checkbox, loses nothing to Escape. */
const textField =
  "textarea, input:is(:not([type]), [type=text], [type=search], [type=email], [type=url], [type=tel], [type=password], [type=number])";

function holdsText(target: EventTarget) {
  const isField = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
  return isField && target.matches(textField) && target.value !== "";
}

/** Keeps Settings open on Escape in a field holding text, so the text isn't thrown away. */
function keepTypedText(event: KeyboardEvent<HTMLDialogElement>) {
  // Preventing Escape's keydown stops the browser asking the dialog to cancel.
  if (event.key === "Escape" && holdsText(event.target)) {
    event.preventDefault();
  }
}

/**
 * Settings, over the page it was opened on: the sections `user` may open beside the pane of the one
 * `section` asks for. It opens as it mounts, and its owner unmounts it once the address stops asking
 * for it.
 */
export function SettingsModal({ section, user }: { section: Section; user: User | undefined }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const close = useCloseSettings();
  const groups = groupsFor(user);
  const shown = openSection(groups, section);
  // A layout effect, so its cleanup closes the dialog while it's still in the page: only then does
  // the browser hand focus back to what opened Settings.
  useLayoutEffect(() => {
    const opened = dialog.current;
    // Strict Mode runs this twice, and a dialog that's open throws on a second `showModal()`.
    if (opened?.open === false) {
      opened.showModal();
    }
    return () => opened?.close();
  }, []);
  return (
    // ponytail: the native modal <dialog> traps focus, makes the page inert and returns focus
    // itself. Its backdrop and it contain scrolling, so the page behind stays put.
    <dialog
      ref={dialog}
      aria-labelledby={titleId}
      onKeyDown={keepTypedText}
      // Escape and the back gesture leave as the close button does, by the address.
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
      onClose={() => {
        // Strict Mode's extra cleanup closes it and opens it again before this arrives.
        if (dialog.current?.open === false) {
          close();
        }
      }}
      className="group/settings inset-0 m-auto size-auto max-h-none max-w-none flex-col overflow-hidden overscroll-contain bg-background text-foreground shadow-lg outline-none text-sm-dense backdrop:overflow-hidden backdrop:overscroll-contain backdrop:bg-scrim open:flex md:inset-4 md:max-h-125 md:max-w-175 md:flex-row md:rounded-lg md:border lg:max-w-200"
    >
      <SettingsSide groups={groups} section={shown} user={user} />
      {/* Wider, the sections run the full height beside the title and the pane. A phone drops
          this box so its title can sit over the strip of sections. */}
      <div className="flex min-h-0 min-w-0 flex-1 flex-col max-md:contents">
        <SettingsHeader titleId={titleId} section={shown} onClose={close} />
        <SettingsPane key={shown} section={shown} user={user} />
      </div>
    </dialog>
  );
}
