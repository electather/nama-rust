/** The texts of the reasons that decided how a plan plays. */
export function Reasons({ texts }: { texts: string[] }) {
  if (texts.length === 0) {
    return null;
  }
  return (
    <ul className="mt-1 space-y-1 text-muted-foreground">
      {texts.map((text) => (
        <li key={text}>{text}</li>
      ))}
    </ul>
  );
}
