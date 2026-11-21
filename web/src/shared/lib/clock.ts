/** Seconds as a clock, such as "4:05" or "1:02:03", with the hours even when 0 if `hours`. */
export function clock(seconds: number, hours = false) {
  const whole = Math.floor(seconds);
  const [wholeHours, minutes, rest] = [
    Math.floor(whole / 3600),
    Math.floor(whole / 60) % 60,
    whole % 60,
  ];
  const showHours = hours || wholeHours > 0;
  const mmss = `${String(minutes).padStart(showHours ? 2 : 1, "0")}:${String(rest).padStart(2, "0")}`;
  return showHours ? `${wholeHours}:${mmss}` : mmss;
}
