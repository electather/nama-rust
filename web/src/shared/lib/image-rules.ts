/** What a kind of image may be. */
export interface ImageRules {
  /** The file types the picker offers. */
  accept: string;
  /** Why the Server refused an upload or removal, by its HTTP status. */
  refusals: Partial<Record<number, string>>;
}
