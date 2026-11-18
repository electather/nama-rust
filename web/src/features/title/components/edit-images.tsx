import { useRouteContext } from "@tanstack/react-router";
import { useId, useRef, useState } from "react";

import { EditImagesDialog } from "@/features/title/components/edit-images-dialog";
import type { TitleImages } from "@/features/title/lib/title-images";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";
import { Button } from "@/shared/ui/button";

/**
 * With Manage Libraries, "Edit images": a dialog showing a title's `images`, each with a way to
 * upload a Custom Image in its place, or pick one from the title's `artwork` lists, and to remove
 * one. The lists are asked for each time it opens. The page shows a change as soon as the Server
 * has it; other people's pages, the next time they load.
 */
export function EditImages({ title, images }: TitleImages & { title: string }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  const dialog = useRef<HTMLDialogElement>(null);
  const id = useId();
  // How many times it has opened, which keys the list, so each opening asks for the artwork anew.
  const [opened, setOpened] = useState(0);
  if (!may(signedIn.user, Permission.MANAGE_LIBRARIES)) {
    return null;
  }
  function open() {
    dialog.current?.showModal();
    setOpened(opened + 1);
  }
  return (
    <>
      <Button variant="ghost" aria-haspopup="dialog" onClick={open}>
        Edit images
      </Button>
      <EditImagesDialog ref={dialog} id={id} title={title} images={images} opened={opened} />
    </>
  );
}
