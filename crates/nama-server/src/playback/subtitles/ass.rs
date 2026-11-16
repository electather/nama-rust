//! ASS and SSA subtitles as plain WebVTT cues. Only the words of their dialogue are kept: their
//! styling, positions and drawings are dropped, as burning them in is what keeps them.

/// The dialogue of `ass`, a script as ffmpeg's `ass` muxer writes it, as WebVTT: a cue for each
/// line with words, by start time.
pub(super) fn to_webvtt(ass: &str) -> String {
    let mut cues: Vec<_> = ass.lines().filter_map(cue).collect();
    // WebVTT's cues go by start time. A script's needn't, and ffmpeg keeps the script's order.
    cues.sort_by(|(start, _), (other, _)| start.cmp(other));
    let cues: String = cues.into_iter().map(|(_, cue)| cue).collect();
    format!("WEBVTT\n\n{cues}")
}

/// The start time and WebVTT cue for `line`, `None` unless it's a `Dialogue` line with words to
/// show.
fn cue(line: &str) -> Option<(String, String)> {
    let fields: Vec<_> = line.strip_prefix("Dialogue:")?.splitn(10, ',').collect();
    // ffmpeg writes dialogue in its script's `Format` order. Nearly every script, SSA's included,
    // has the usual ten fields, the last, the text, alone holding commas; ffmpeg's own WebVTT
    // encoder drops any other script's dialogue too.
    let [_, start, end, _, _, _, _, _, _, text] = fields[..] else {
        return None;
    };
    let words = words(text);
    let start = time(start);
    let cue = format!("{start} --> {}\n{words}\n\n", time(end));
    (!words.is_empty()).then_some((start, cue))
}

/// An ASS time, `H:MM:SS.cc` as ffmpeg writes it, as a WebVTT one, `HH:MM:SS.mmm`, which sorts as
/// text.
// ponytail: ffmpeg's `ass` muxer writes no time past 9:59:59.99, so cues after ten hours collapse
// there; take the times from ffmpeg's packets if a ten-hour ASS Track turns up.
fn time(ass: &str) -> String {
    format!("{ass:0>11}0")
}

/// The words `text` shows, without its override blocks or what it draws, line by line and
/// escaped for a cue.
fn words(text: &str) -> String {
    let mut shown = String::new();
    let mut drawing = false;
    let mut rest = text;
    loop {
        let (words, block) = rest.split_once('{').unwrap_or((rest, ""));
        if !drawing {
            shown.push_str(words);
        }
        let Some((tags, after)) = block.split_once('}') else {
            break;
        };
        drawing = draws(tags).unwrap_or(drawing);
        rest = after;
    }
    // `\n` breaks a line only where the script wraps by hand, which a browser doesn't do.
    let broken = (shown.replace("\\N", "\n").replace("\\n", " ")).replace("\\h", "\u{a0}");
    let escaped = (broken.replace('&', "&amp;").replace('<', "&lt;")).replace('>', "&gt;");
    // A blank line would end the cue.
    let lines: Vec<_> = escaped
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    lines.join("\n")
}

/// Whether the override tags `tags` leave drawing on, `None` if they don't say: `\p1` and up turn
/// it on, `\p0` off. `\pos` and `\pbo` are other tags.
fn draws(tags: &str) -> Option<bool> {
    let scale = tags
        .rsplit('\\')
        .find_map(|tag| tag.strip_prefix('p')?.trim().parse::<u32>().ok());
    scale.map(|scale| scale > 0)
}

#[cfg(test)]
#[path = "ass_tests.rs"]
mod tests;
