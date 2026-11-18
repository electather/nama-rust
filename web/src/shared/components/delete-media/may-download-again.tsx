// "en-GB" for "Radarr and Sonarr", without the serial comma, as the rest of the app writes lists.
const names = new Intl.ListFormat("en-GB", { type: "conjunction" });

/** Says which Fulfillers couldn't be told a delete of `title` happened, so they may download it again. */
export function MayDownloadAgain({ title, fulfillers }: { title: string; fulfillers: string[] }) {
  return (
    <output className="block basis-full text-sm text-pretty empty:hidden">
      {fulfillers.length > 0 &&
        `${names.format(fulfillers)} couldn’t be told about the delete, so ${fulfillers.length === 1 ? "it" : "they"} may download ${title} again.`}
    </output>
  );
}
