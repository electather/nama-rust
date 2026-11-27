import { type KeyboardEvent, useId, useLayoutEffect, useRef } from "react";

import { HubSettings } from "@/features/settings/components/hub-settings";
import { SettingsBody } from "@/features/settings/components/settings-body";
import { useCloseOnScrim } from "@/features/settings/hooks/use-close-on-scrim";
import { useCloseSettings } from "@/features/settings/hooks/use-close-settings";
import { holdsText } from "@/features/settings/lib/holds-text";
import { phoneWidth } from "@/features/settings/lib/phone-width";
import { groupsFor } from "@/features/settings/lib/sections";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import type { SettingsPage } from "@/shared/lib/settings-address";

/** Keeps Settings open on Escape in a text field while it or its form holds text. */
function keepTypedText(event: KeyboardEvent<HTMLDialogElement>) {
  // Preventing Escape's keydown stops the browser asking the dialog to cancel.
  if (event.key === "Escape" && holdsText(event.target)) {
    event.preventDefault();
  }
}

/**
 * Settings, over the page it was opened on: the sections `user` may open, or `atHub` the Account's
 * and those of the Server it picks, beside the pane of the one `section` asks for. It opens as it
 * mounts, and its owner unmounts it once the address stops asking for it.
 */
export function SettingsModal({
  section,
  user,
  atHub,
}: {
  section: SettingsPage;
  user: User | undefined;
  atHub: boolean;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const close = useCloseSettings();
  const header = { titleId, onClose: close };
  useCloseOnScrim(dialog, close);
  // A layout effect, so its cleanup closes the dialog while it's still in the page: only then does
  // the browser hand focus back to what opened Settings.
  useLayoutEffect(() => {
    const opened = dialog.current;
    // Strict Mode runs this twice, and a dialog that's open throws on a second `showModal()`.
    if (opened?.open === false) {
      opened.showModal();
      // Chosen here, after `showModal()` focused the first control: React's `autoFocus` would fire
      // while the dialog is still closed. Wider, Settings starts on Search, so typing finds a setting
      // at once; it's found by its name, as a Section's pane holds comboboxes too. On a phone, Search
      // would open the keyboard over Settings.
      const start = phoneWidth()
        ? opened
        : opened.querySelector<HTMLElement>('[aria-label="Search settings"]');
      start?.focus();
    }
    return () => opened?.close();
  }, []);
  return (
    // ponytail: the native modal <dialog> traps focus, makes the page inert and returns focus
    // itself. Its backdrop and it contain scrolling, so the page behind stays put.
    // On a phone it's a sheet 11/12 of the window tall that slides up from the bottom, so the page
    // it was opened on shows above it.
    // Wider than a phone it fills the window 16px in from its edges (`md:inset-4`), up to 760px
    // tall, then sits centred.
    <dialog
      ref={dialog}
      aria-labelledby={titleId}
      // Focusable, so a phone can start on the dialog itself.
      tabIndex={-1}
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
      className="group/settings inset-0 m-auto size-auto max-h-none max-w-none flex-col overflow-hidden overscroll-contain bg-background text-foreground shadow-lg outline-none text-sm-dense backdrop:overflow-hidden backdrop:overscroll-contain backdrop:bg-scrim open:flex motion-reduce:transition-none max-md:mb-0 max-md:h-11/12 max-md:w-full max-md:rounded-t-2xl max-md:border-t max-md:transition-transform max-md:duration-300 max-md:ease-out-expo md:inset-4 md:max-h-190 md:max-w-175 md:flex-row md:rounded-lg md:border lg:max-w-200 xl:max-w-5xl max-md:motion-safe:starting:open:translate-y-full"
    >
      {atHub ? (
        <HubSettings section={section} header={header} />
      ) : (
        <SettingsBody
          groups={groupsFor(user, false)}
          section={section}
          user={user}
          header={header}
        />
      )}
    </dialog>
  );
}
