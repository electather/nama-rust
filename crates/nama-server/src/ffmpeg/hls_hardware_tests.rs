use super::{SOFTWARE, ZSCALE_TONE_MAP, on_default_gpu, transcode_args, value_of};
use crate::ffmpeg::{Accel, ToneMapper, VideoConversion, VideoLimit};

/// What a Transcode on `accel`, tone-mapping with `tone_map`, starts ffmpeg with up to the
/// keyframes it forces, which the software tests pin from there on, as words split at spaces.
/// `None` for tone-mapping on the GPU of an API with no filter for it, which the plan never
/// asks for: only an API with one can pass its tone-map test at startup.
fn pinned_hardware_args(accel: Accel, tone_map: Option<ToneMapper>) -> Option<String> {
    const IN_BAND_HEADERS: &str = "-bsf:v dump_extra=freq=keyframe -tag:v avc3";
    let args = match (accel, tone_map) {
        (Accel::Nvenc, None) => format!(
            "-v error -nostdin -init_hw_device cuda=nama -filter_hw_device nama \
             -hwaccel cuda -hwaccel_device nama -hwaccel_output_format cuda \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf format=nv12|p010le|cuda,hwupload,scale_cuda=format=nv12 \
             -c:v h264_nvenc -g 10000000 -forced-idr 1 -no-scenecut 1 {IN_BAND_HEADERS}"
        ),
        (Accel::Nvenc, Some(ToneMapper::Software)) => format!(
            "-v error -nostdin -init_hw_device cuda=nama -filter_hw_device nama \
             -hwaccel cuda -hwaccel_device nama \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf {ZSCALE_TONE_MAP},format=nv12 \
             -c:v h264_nvenc -g 10000000 -forced-idr 1 -no-scenecut 1 {IN_BAND_HEADERS}"
        ),
        (Accel::Qsv, None) => format!(
            "-v error -nostdin -init_hw_device qsv=nama -filter_hw_device nama \
             -hwaccel qsv -hwaccel_device nama -hwaccel_output_format qsv \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf format=nv12|p010le|qsv,hwupload=extra_hw_frames=64,scale_qsv=format=nv12 \
             -c:v h264_qsv -g 10000000 -forced_idr 1 {IN_BAND_HEADERS}"
        ),
        (Accel::Qsv, Some(ToneMapper::Hardware)) => format!(
            "-v error -nostdin -init_hw_device qsv=nama -filter_hw_device nama \
             -hwaccel qsv -hwaccel_device nama -hwaccel_output_format qsv \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf format=nv12|p010le|qsv,hwupload=extra_hw_frames=64,\
             vpp_qsv=tonemap=1:format=nv12,scale_qsv=format=nv12 \
             -c:v h264_qsv -g 10000000 -forced_idr 1 {IN_BAND_HEADERS}"
        ),
        (Accel::Qsv, Some(ToneMapper::Software)) => format!(
            "-v error -nostdin -init_hw_device qsv=nama -filter_hw_device nama \
             -hwaccel qsv -hwaccel_device nama \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf {ZSCALE_TONE_MAP},format=nv12,hwupload=extra_hw_frames=64 \
             -c:v h264_qsv -g 10000000 -forced_idr 1 {IN_BAND_HEADERS}"
        ),
        (Accel::Vaapi, None) => format!(
            "-v error -nostdin -init_hw_device vaapi=nama -filter_hw_device nama \
             -hwaccel vaapi -hwaccel_device nama -hwaccel_output_format vaapi \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf format=nv12|p010le|vaapi,hwupload,scale_vaapi=format=nv12 \
             -c:v h264_vaapi -g 10000000 {IN_BAND_HEADERS}"
        ),
        (Accel::Vaapi, Some(ToneMapper::Hardware)) => format!(
            "-v error -nostdin -init_hw_device vaapi=nama -filter_hw_device nama \
             -hwaccel vaapi -hwaccel_device nama -hwaccel_output_format vaapi \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf format=nv12|p010le|vaapi,hwupload,\
             tonemap_vaapi=format=nv12:t=bt709:m=bt709:p=bt709,scale_vaapi=format=nv12 \
             -c:v h264_vaapi -g 10000000 {IN_BAND_HEADERS}"
        ),
        (Accel::Vaapi, Some(ToneMapper::Software)) => format!(
            "-v error -nostdin -init_hw_device vaapi=nama -filter_hw_device nama \
             -hwaccel vaapi -hwaccel_device nama \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf {ZSCALE_TONE_MAP},format=nv12,hwupload \
             -c:v h264_vaapi -g 10000000 {IN_BAND_HEADERS}"
        ),
        (Accel::VideoToolbox, None) => format!(
            "-v error -nostdin -hwaccel videotoolbox \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 -vf format=nv12 \
             -c:v h264_videotoolbox -g 10000000 {IN_BAND_HEADERS}"
        ),
        (Accel::VideoToolbox, Some(ToneMapper::Software)) => format!(
            "-v error -nostdin -hwaccel videotoolbox \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf {ZSCALE_TONE_MAP},format=nv12 \
             -c:v h264_videotoolbox -g 10000000 {IN_BAND_HEADERS}"
        ),
        (Accel::Nvenc | Accel::VideoToolbox, Some(ToneMapper::Hardware)) => return None,
    };
    Some(args)
}

#[test]
fn a_hardware_transcode_runs_with_its_apis_arguments_for_each_tone_mapper() {
    let tone_mappers = [None, Some(ToneMapper::Software), Some(ToneMapper::Hardware)];
    let cases =
        (Accel::ALL.into_iter()).flat_map(|accel| tone_mappers.map(|tone_map| (accel, tone_map)));
    for (accel, tone_map) in cases {
        let Some(expected) = pinned_hardware_args(accel, tone_map) else {
            assert!(
                !accel.has_tone_map_filter(),
                "{accel:?} can tone-map on its GPU"
            );
            continue;
        };
        let video = VideoConversion {
            hardware: Some(on_default_gpu(accel)),
            tone_map,
            // As the plan sets it on hardware, so software can carry on if the hardware fails.
            in_band_headers: true,
            ..SOFTWARE
        };

        let args = transcode_args(video);

        let forced = (args.iter())
            .position(|arg| arg == "-force_key_frames")
            .expect("a Transcode forces keyframes");
        assert_eq!(
            args[..forced],
            expected.split(' ').collect::<Vec<_>>(),
            "{accel:?} tone-mapped by {tone_map:?}"
        );
    }
}

#[test]
fn a_playback_limit_on_hardware_scales_the_picture_on_its_gpu_if_it_can() {
    for accel in Accel::ALL {
        let limited = VideoConversion {
            hardware: Some(on_default_gpu(accel)),
            limit: Some(VideoLimit {
                size: Some((1280, 720)),
                bits_per_second: 4_000_000,
            }),
            in_band_headers: true,
            ..SOFTWARE
        };

        let args = transcode_args(limited);

        let filters = match accel {
            Accel::Nvenc => "format=nv12|p010le|cuda,hwupload,scale_cuda=w=1280:h=720:format=nv12",
            Accel::Qsv => {
                "format=nv12|p010le|qsv,hwupload=extra_hw_frames=64,\
                 scale_qsv=w=1280:h=720:format=nv12"
            }
            Accel::Vaapi => {
                "format=nv12|p010le|vaapi,hwupload,scale_vaapi=w=1280:h=720:format=nv12"
            }
            Accel::VideoToolbox => "scale=1280:720,format=nv12",
        };
        assert_eq!(value_of(&args, "-vf"), Some(filters), "{accel:?}");
    }
}
