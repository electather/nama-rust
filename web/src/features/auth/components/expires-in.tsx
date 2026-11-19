import { useEffect, useState } from "react";

/** "Expires in m:ss", counting down to `expiresAt` once a second. */
export function ExpiresIn({ expiresAt }: { expiresAt: number }) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => {
      clearInterval(timer);
    };
  }, []);
  const seconds = Math.max(0, Math.ceil((expiresAt - now) / 1000));
  return (
    <span className="text-xs text-muted-foreground tabular-nums">
      Expires in {Math.floor(seconds / 60)}:{String(seconds % 60).padStart(2, "0")}
    </span>
  );
}
