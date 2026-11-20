//! What the lasting steps allow past the plan they shaped: the choices it can ever honour.

use std::collections::BTreeSet;

use super::{
    Facts, Method, Offers, Track,
    choices::burns_in,
    codecs::{file_audio, is_audio},
    limits::{fitted, picture},
};
use crate::server_settings::PlaybackLimit;

/// What a plan for `facts` can ever honour, with only the methods in `lasting` allowed.
pub(super) fn offers(facts: &Facts, lasting: &BTreeSet<Method>) -> Offers {
    let Some(probe) = &facts.probe else {
        return Offers::default();
    };
    let transcodes = lasting.contains(&Method::Transcode);
    let places = |keep: &dyn Fn(&Track) -> bool| {
        (probe.tracks.iter().enumerate())
            .filter(|(_, track)| keep(track))
            .map(|(place, _)| place)
            .collect()
    };
    // Only the file's own audio plays without a Remux or a Transcode.
    let audio = if transcodes || lasting.contains(&Method::Remux) {
        places(&is_audio)
    } else {
        file_audio(probe).into_iter().collect()
    };
    if !transcodes {
        return Offers {
            audio,
            ..Offers::default()
        };
    }
    let picture = picture(facts);
    Offers {
        audio,
        burn_in: places(&|track| burns_in(&facts.device, probe, track)),
        limits: (PlaybackLimit::ALL.into_iter())
            .filter(|&limit| picture.and_then(|picture| fitted(picture, limit)).is_some())
            .collect(),
    }
}
