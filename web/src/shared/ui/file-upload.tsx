import { FileUpload as FileUploadPrimitive, useFileUploadContext } from "@ark-ui/react/file-upload";
import { cn } from "cn";
import { Upload } from "lucide-react";
import type { ComponentProps } from "react";

import { buttonVariants } from "@/shared/ui/button";

// shadcn.io's file upload, on Ark UI as Base UI has none, trimmed to the parts we use: the dropzone
// with its icon, title, description and the look of its browse button. DESIGN.md, "Inputs". Add its
// FileUploadList and item parts when a screen collects files before sending them.

/** The ids a dropzone looks its title and description up by, from the upload they're all under. */
function useNames() {
  const title = useFileUploadContext().getLabelProps().id ?? "";
  return { title, description: `${title}-description` };
}

/**
 * The upload around its parts, with the hidden file input: give it `accept`, and `onFileAccept` and
 * `onFileReject` for what's picked or dropped. It sends nothing itself. Unless `acceptedFiles` says
 * otherwise it keeps the file it took, and won't take that one again.
 */
function FileUpload({ children, ...props }: FileUploadPrimitive.RootProps) {
  return (
    <FileUploadPrimitive.Root data-slot="file-upload" {...props}>
      {children}
      <FileUploadPrimitive.HiddenInput />
    </FileUploadPrimitive.Root>
  );
}

/**
 * The dashed box: a file dropped on it is taken, and a click, Enter or Space on it opens the file
 * dialog. The `FileUploadTitle` in it names it, and the `FileUploadDescription` describes it.
 */
function FileUploadDropzone({ className, ...props }: FileUploadPrimitive.DropzoneProps) {
  const names = useNames();
  return (
    <FileUploadPrimitive.Dropzone
      data-slot="file-upload-dropzone"
      aria-labelledby={names.title}
      aria-describedby={names.description}
      className={cn(
        "flex cursor-pointer flex-col items-center gap-1 rounded-xl border border-dashed border-input px-6 py-8 text-center transition-colors outline-none hover:bg-muted/50 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 data-dragging:border-ring data-dragging:bg-muted motion-reduce:transition-none data-disabled:cursor-default data-disabled:bg-muted data-disabled:text-muted-foreground",
        className,
      )}
      {...props}
    />
  );
}

/** The upload mark over a dropzone's words, on a tile as a row's icon is. */
function FileUploadDropzoneIcon() {
  return (
    <span
      aria-hidden
      data-slot="file-upload-dropzone-icon"
      className="mb-2 grid size-10 place-items-center rounded-lg bg-muted inset-ring inset-ring-border"
    >
      <Upload strokeWidth={1.5} className="size-5" />
    </span>
  );
}

/** What to upload, in a few words. It names the dropzone it's in. */
function FileUploadTitle({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="file-upload-title"
      id={useNames().title}
      className={cn("text-sm font-medium", className)}
      {...props}
    />
  );
}

/** What the upload takes, such as its file types and sizes. */
function FileUploadDescription({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="file-upload-description"
      id={useNames().description}
      className={cn("max-w-prose text-sm text-pretty text-muted-foreground", className)}
      {...props}
    />
  );
}

/**
 * The words in a dropzone that say a click browses, shaped as a button. Only the look of shadcn.io's
 * FileUploadTrigger there: the dropzone is already a button, and one inside another is lost on a
 * screen reader and is a second stop for the same thing on the keyboard.
 */
function FileUploadBrowse({ className, ...props }: ComponentProps<"span">) {
  return (
    <span
      aria-hidden
      data-slot="file-upload-browse"
      className={cn(buttonVariants({ variant: "outline" }), "mt-3", className)}
      {...props}
    />
  );
}

export {
  FileUpload,
  FileUploadBrowse,
  FileUploadDescription,
  FileUploadDropzone,
  FileUploadDropzoneIcon,
  FileUploadTitle,
};
