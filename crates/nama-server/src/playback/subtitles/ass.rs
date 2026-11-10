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
    // ffmpeg writes ASS and SSA dialogue with the same ten fields; only the last, the text, may
    // hold commas.
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
mod tests {
    use super::to_webvtt;

    fn script(dialogue: &[&str]) -> String {
        let lines = dialogue.iter().map(|line| format!("Dialogue: {line}\n"));
        format!(
            "[Script Info]\nScriptType: v4.00+\n\n[Events]\n{}",
            lines.collect::<String>()
        )
    }

    #[test]
    fn dialogue_keeps_its_words_and_line_breaks_but_not_its_override_tags() {
        let ass = script(&[
            r"0,0:00:01.50,0:00:02.00,Default,,0,0,0,,{\an8\pos(10,10)\b1}Hi,{\b0} you\Nthere",
            r"0,10:00:00.00,10:00:01.25,Default,,0,0,0,,soft\nbreak and\hhard space",
        ]);

        assert_eq!(
            to_webvtt(&ass),
            "WEBVTT\n\n\
             00:00:01.500 --> 00:00:02.000\nHi, you\nthere\n\n\
             10:00:00.000 --> 10:00:01.250\nsoft break and\u{a0}hard space\n\n"
        );
    }

    #[test]
    fn what_dialogue_draws_is_dropped_and_a_cue_with_nothing_else_too() {
        let ass = script(&[
            r"0,0:00:00.00,0:00:01.00,Default,,0,0,0,,{\p1 }m 0 0 l 100 0 100 100{\p0}",
            r"0,0:00:00.00,0:00:01.00,Default,,0,0,0,,Look{\p2\pos(1,1)}m 0 0 l 9 9{\pos(2,2)}l 1 1{\p0} here",
        ]);

        assert_eq!(
            to_webvtt(&ass),
            "WEBVTT\n\n00:00:00.000 --> 00:00:01.000\nLook here\n\n"
        );
    }

    #[test]
    fn cues_go_by_start_time_whatever_the_scripts_order() {
        let ass = script(&[
            r"0,0:00:02.00,0:00:03.00,Default,,0,0,0,,Later",
            r"0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Sooner",
        ]);

        assert_eq!(
            to_webvtt(&ass),
            "WEBVTT\n\n\
             00:00:01.000 --> 00:00:02.000\nSooner\n\n\
             00:00:02.000 --> 00:00:03.000\nLater\n\n"
        );
    }

    #[test]
    fn dialogue_is_escaped_and_never_holds_a_blank_line() {
        let ass = script(&[r"0,0:00:00.00,0:00:01.00,Default,,0,0,0,,<b> & --> \N\N  ok"]);

        assert_eq!(
            to_webvtt(&ass),
            "WEBVTT\n\n00:00:00.000 --> 00:00:01.000\n&lt;b&gt; &amp; --&gt;\nok\n\n"
        );
    }
}
