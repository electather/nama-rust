/** "All", "Crime", "Crime +2": the genres turned on, short enough for a phone's bar. */
export function chosenGenres([first, ...more]: string[]) {
  if (first === undefined) {
    return "All";
  }
  return more.length === 0 ? first : `${first} +${more.length}`;
}
