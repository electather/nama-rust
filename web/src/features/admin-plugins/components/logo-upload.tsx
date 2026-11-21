import {
  FileUpload,
  FileUploadBrowse,
  FileUploadDescription,
  FileUploadDropzone,
  FileUploadDropzoneIcon,
  FileUploadTitle,
} from "@/shared/ui/file-upload";

/** Kept empty: a logo is sent as it's chosen, so the same file can be chosen again after a refusal. */
const NONE: File[] = [];

/**
 * The box a Plugin's logo is dropped on or browsed for, of a type in `accept`, which hands it to
 * `onFile` at once. Nothing is taken while one is `pending`.
 */
export function LogoUpload({
  accept,
  pending,
  onFile,
}: {
  accept: string;
  pending: boolean;
  onFile: (file: File) => void;
}) {
  function send(file: File | undefined) {
    if (file) {
      onFile(file);
    }
  }
  return (
    <FileUpload
      accept={accept}
      acceptedFiles={NONE}
      disabled={pending}
      onFileAccept={({ files }) => {
        send(files[0]);
      }}
      // A file the browser turns away still goes to the Server, which says why.
      onFileReject={({ files }) => {
        send(files[0]?.file);
      }}
    >
      <FileUploadDropzone>
        <FileUploadDropzoneIcon />
        <FileUploadTitle>Upload a logo</FileUploadTitle>
        <FileUploadDescription>
          Drop an image here, or browse for one. JPEG, PNG or WebP up to 20 MB, or SVG up to 64 KB.
        </FileUploadDescription>
        <FileUploadBrowse>Browse files</FileUploadBrowse>
      </FileUploadDropzone>
    </FileUpload>
  );
}
