use super::{
    BurnIn, FromCut, ImageSubtitle, SOFTWARE, Transcode, VideoConversion, args, ffmpeg, plan,
    value_of,
};

#[test]
fn a_pgs_sidecar_is_burned_in_from_a_second_input_moved_back_to_its_first_picture_and_never_seeked()
{
    let dir = tempfile::tempdir().unwrap();
    let sup = dir.path().join("Alien.en.sup").display().to_string();
    let five_seconds_in_90_khz_ticks = 450_000_u32;
    let segment_header = [
        &b"PG"[..],
        &five_seconds_in_90_khz_ticks.to_be_bytes(),
        &[0; 7],
    ];
    std::fs::write(&sup, segment_header.concat()).unwrap();
    let transcode = Transcode {
        audio: None,
        video: VideoConversion {
            burn_in: Some(BurnIn::Image {
                subtitle: ImageSubtitle::Sidecar(sup.clone()),
                picture: None,
            }),
            ..SOFTWARE
        },
    };
    let from_start = FromCut {
        n: 0,
        cuts: &[0, 6000],
    };
    let seeking = FromCut {
        n: 1,
        cuts: &[6000],
    };

    for (from, video_input) in [(from_start, vec![]), (seeking, vec!["-ss", "6.000"])] {
        let run = ffmpeg("7.1.1").transcode("alien.mkv", &transcode, &plan(), from);
        let args = args(&run.unwrap());

        let copyts = args.iter().position(|arg| arg == "-copyts").unwrap();
        let first_input = args
            .iter()
            .position(|arg| arg == "-ss" || arg == "-i")
            .unwrap();
        let inputs = [
            video_input,
            vec!["-i", "alien.mkv", "-itsoffset", "5.000", "-i", &sup],
        ];
        assert_eq!(args[first_input..copyts], inputs.concat(), "{args:?}");
        let graph = value_of(&args, "-filter_complex").unwrap();
        assert!(graph.contains("[1:s:0]"), "{graph}");
    }
}
