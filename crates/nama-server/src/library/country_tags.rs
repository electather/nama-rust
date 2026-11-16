//! Country tags ending a Show's name, as Sonarr names same-name Shows apart: `The Office (US)`.
//! The scanner keeps them in the stored name and Matching reads them.

use std::sync::LazyLock;

use regex::Regex;

/// A name ending in a bracket of two capital letters, which may be a country tag.
#[expect(
    clippy::expect_used,
    reason = "the pattern is a constant, and the country tag tests compile it"
)]
static COUNTRY_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<title>.+?)\s*\((?P<code>[A-Z]{2})\)$").expect("a constant pattern compiles")
});

/// Every ISO 3166-1 alpha-2 code, so a bracket such as `(II)` stays part of the title.
const COUNTRY_CODES: &str = "\
    AD AE AF AG AI AL AM AO AQ AR AS AT AU AW AX AZ BA BB BD BE BF BG BH BI BJ BL BM BN BO BQ \
    BR BS BT BV BW BY BZ CA CC CD CF CG CH CI CK CL CM CN CO CR CU CV CW CX CY CZ DE DJ DK DM \
    DO DZ EC EE EG EH ER ES ET FI FJ FK FM FO FR GA GB GD GE GF GG GH GI GL GM GN GP GQ GR GS \
    GT GU GW GY HK HM HN HR HT HU ID IE IL IM IN IO IQ IR IS IT JE JM JO JP KE KG KH KI KM KN \
    KP KR KW KY KZ LA LB LC LI LK LR LS LT LU LV LY MA MC MD ME MF MG MH MK ML MM MN MO MP MQ \
    MR MS MT MU MV MW MX MY MZ NA NC NE NF NG NI NL NO NP NR NU NZ OM PA PE PF PG PH PK PL PM \
    PN PR PS PT PW PY QA RE RO RS RU RW SA SB SC SD SE SG SH SI SJ SK SL SM SN SO SR SS ST SV \
    SX SY SZ TC TD TF TG TH TJ TK TL TM TN TO TR TT TV TW TZ UA UG UM US UY UZ VA VC VE VG VI \
    VN VU WF WS YE YT ZA ZM ZW";

/// The title in a Show's name and the country its country tag names, as TMDB codes it: `The
/// Office (UK)` is `("The Office", "GB")`. `None` for a name with no country tag.
pub(crate) fn country_tag(show: &str) -> Option<(&str, &str)> {
    let captures = COUNTRY_TAG.captures(show)?;
    let title = captures.name("title")?.as_str();
    let code = match captures.name("code")?.as_str() {
        // Sonarr's titles, from TheTVDB, say UK where ISO and TMDB say GB.
        "UK" => "GB",
        code => code,
    };
    COUNTRY_CODES
        .split_whitespace()
        .any(|known| known == code)
        .then_some((title, code))
}
