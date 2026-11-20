import { useRouteContext } from "@tanstack/react-router";
import { type Ref, useId, useImperativeHandle, useRef, useState } from "react";

import { EditImagesDialog } from "@/features/title/components/edit-images-dialog";
import type { TitleImages } from "@/features/title/lib/title-images";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import type { Opener } from "@/shared/lib/opener";
import { may } from "@/shared/lib/permissions";

/**
 * With Manage Libraries, "Edit images": a dialog showing a title's `images`, each with a way to
 * upload a Custom Image in its place, or pick one from the title's `artwork` lists, and to remove
 * one. The lists are asked for each time it opens. The page shows a change as soon as the Server
 * has it; other people's pages, the next time they load. Only `ref` opens it, from a menu's item,
 * since a title's page keeps Edit images in its "…" menu.
 */
export function EditImages({
  title,
  images,
  ref,
}: TitleImages & { title: string; ref: Ref<Opener> }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  const dialog = useRef<HTMLDialogElement>(null);
  const id = useId();
  // How many times it has opened, which keys the list, so each opening asks for the artwork anew.
  const [opened, setOpened] = useState(0);
  const open = () => {
    dialog.current?.showModal();
    setOpened(opened + 1);
  };
  useImperativeHandle(ref, () => ({ open }));
  if (!may(signedIn.user, Permission.MANAGE_LIBRARIES)) {
    return null;
  }
  return <EditImagesDialog ref={dialog} id={id} title={title} images={images} opened={opened} />;
}
