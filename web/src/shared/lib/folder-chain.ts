// shortcut: a Windows folder typed in another case than the Server lists it, such as `c:\media`
// for `C:\Media`, stops the tree there; match without case if Windows Admins type paths so.
/**
 * The folders from the top of the Server's disks down to `path`, each as `ListServerFolders`
 * names it: `/media` then `/media/films` for `/media/films/`, or `C:\` then `C:\Media` for
 * `c:\Media`. A path that isn't absolute has none.
 */
export function folderChain(path: string) {
  const names = path.split(/[\\/]/u).filter((name) => name !== "");
  const drive = /^(?<letter>[a-z]):(?:[\\/]|$)/iu.exec(path)?.groups?.["letter"];
  if (drive !== undefined) {
    const root = `${drive.toUpperCase()}:\\`;
    const below = names.slice(1);
    return [root, ...below.map((_, at) => root + below.slice(0, at + 1).join("\\"))];
  }
  return path.startsWith("/") ? names.map((_, at) => `/${names.slice(0, at + 1).join("/")}`) : [];
}
