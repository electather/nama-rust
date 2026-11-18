/** What a kind of image may be, and what to read again once it changes. */
export interface ImageRules {
  /** The file types the picker offers. */
  accept: string;
  /** Why the Server refused an upload or removal, by its HTTP status. */
  refusals: Partial<Record<number, string>>;
  /** The queries that send its URL; `[]` is every query. */
  queryKey: readonly unknown[];
}
