/** How far the arrow keys and the media keys seek. */
export const SEEK_STEP_SECONDS = 10;

/** Seeks `seconds` forward, or back if negative, but not before the start. */
export function seekBy(video: HTMLVideoElement, seconds: number) {
  video.currentTime = Math.max(0, video.currentTime + seconds);
}

/** Plays `video` if it's paused, or else pauses it. */
export function togglePlay(video: HTMLVideoElement) {
  if (video.paused) {
    play(video);
  } else {
    video.pause();
  }
}

/** Plays `video`. A refused play, such as autoplay the browser blocked, leaves it paused with Play showing. */
export function play(video: HTMLVideoElement) {
  void tryPlay(video);
}

async function tryPlay(video: HTMLVideoElement) {
  try {
    await video.play();
  } catch {
    // Left paused.
  }
}

/** Mutes `video`, or unmutes it. */
export function toggleMute(video: HTMLVideoElement) {
  video.muted = !video.muted;
}

/** Sets the volume, kept between 0 and 1; at 0 the video is muted, so the controls show it. */
export function setVolume(video: HTMLVideoElement, level: number) {
  // Rounded, so that stepping down by 0.1 lands on 0 and not a float's remainder above it.
  video.volume = Math.min(1, Math.max(0, Math.round(level * 100) / 100));
  video.muted = video.volume === 0;
}

/** Makes `player` full screen, or leaves full screen if something is. */
export function toggleFullscreen(player: HTMLElement | null) {
  if (document.fullscreenElement) {
    void document.exitFullscreen();
  } else {
    void player?.requestFullscreen();
  }
}

/** Seconds as a clock, such as "4:05" or "1:02:03". */
export function clock(seconds: number) {
  const whole = Math.floor(seconds);
  const [hours, minutes, rest] = [
    Math.floor(whole / 3600),
    Math.floor(whole / 60) % 60,
    whole % 60,
  ];
  const mmss = `${String(minutes).padStart(hours > 0 ? 2 : 1, "0")}:${String(rest).padStart(2, "0")}`;
  return hours > 0 ? `${hours}:${mmss}` : mmss;
}
