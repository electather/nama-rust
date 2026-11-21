/** A codec, as ffprobe names it, by the name people know it by, or else in capitals. */
export function codecName(codec: string) {
  return codecNames.get(codec) ?? codec.toUpperCase();
}

const codecNames = new Map([
  ["h264", "H.264"],
  ["ac3", "Dolby Digital"],
  ["eac3", "Dolby Digital Plus"],
  ["truehd", "Dolby TrueHD"],
  ["dts", "DTS"],
  ["opus", "Opus"],
  ["vorbis", "Vorbis"],
  ["subrip", "SRT"],
  ["mov_text", "Text"],
  ["webvtt", "WebVTT"],
  ["hdmv_pgs_subtitle", "PGS"],
  ["dvd_subtitle", "VobSub"],
]);
