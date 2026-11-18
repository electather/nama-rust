/** The subtitle at `url`, or `null` if it can't be fetched. */
export async function fetchSubtitle(url: string) {
  try {
    const response = await fetch(url);
    return response.ok ? await response.arrayBuffer() : null;
  } catch {
    return null;
  }
}

/** Each of `urls`, or `null` if any can't be fetched. */
export async function fetchAll(urls: string[]) {
  const fetched = await Promise.all(
    urls.map(async (url) => {
      const body = await fetchSubtitle(url);
      return body;
    }),
  );
  return fetched.every((body) => body !== null) ? fetched : null;
}
