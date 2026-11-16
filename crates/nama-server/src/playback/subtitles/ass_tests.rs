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
        r"0,1:00:00.00,1:00:01.25,Default,,0,0,0,,soft\nbreak and\hhard space",
    ]);

    assert_eq!(
        to_webvtt(&ass),
        "WEBVTT\n\n\
         00:00:01.500 --> 00:00:02.000\nHi, you\nthere\n\n\
         01:00:00.000 --> 01:00:01.250\nsoft break and\u{a0}hard space\n\n"
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
