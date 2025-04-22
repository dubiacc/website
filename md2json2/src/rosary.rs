use serde::Deserialize;
use std::collections::HashMap;

use crate::{get_string, MetaJson};

/// Holds all the relevant template snippets for rendering the Rosary.
#[derive(Debug, Default, Clone)]
pub struct RosaryTemplates {
    /// The main "tools.rosary.{lang}.html" file content.
    pub main_html: String,
    /// The "tools.rosary.outro.{lang}.html" file content.
    pub outro_html: String,
    /// The "tools.rosary.ourfather.{lang}.html" file content.
    pub ourfather_html: String,
    /// The "tools.rosary.glorybe.{lang}.html" file content.
    pub glorybe_html: String,
    /// The "tools.rosary.fatima.{lang}.html" file content.
    pub fatima_html: String,
    /// The "tools.rosary.nav.{lang}.html" file content.
    pub nav_html: String,
    /// The template for one Hail Mary section (e.g. "tools.rosary.mystery.html").
    pub mystery_section_html: String,
}

/// Deserialized structure for the entire "mysteries.json" file.
///
/// The JSON is typically of the form:
/// ```json
/// {
///   "mysteries": {
///     "1": {
///       "decade": { "en": "...", "de": "..." },
///       "spiritual-fruit": { "en": "...", "de": "..." },
///       "prayers": {
///         "1": { "en": "...", "de": "...", "source": "...", "image": "..." },
///         "2": { ... },
///         ...
///       }
///     },
///     "2": { ... },
///     ...
///   }
/// }
/// ```
#[derive(Debug, Default, Clone, Deserialize)]
pub struct RosaryMysteries {
    pub mysteries: HashMap<String, Mystery>,
}

/// Represents one numbered mystery ("1", "2", ... "15") in `mysteries.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Mystery {
    /// Maps "en"/"de" to the name of the decade, e.g. "The Annunciation".
    pub decade: HashMap<String, String>,

    /// Maps "en"/"de" to the name of the spiritual fruit, e.g. "Humility".
    #[serde(rename = "spiritual-fruit")]
    pub spiritual_fruit: HashMap<String, String>,

    /// Keys are "1" through "10" for the 10 Hail Marys.
    /// Each value is a `RosaryPrayer` describing
    /// the text/source/image for that bead's reflection.
    pub prayers: HashMap<String, RosaryPrayer>,
}

/// Holds the data for each individual prayer in a decade:
///
/// ```json
/// {
///   "en": "In the first Hail Mary, we reflect on ...",
///   "de": "Im ersten Ave Maria betrachten wir ...",
///   "source": "...",
///   "image": "unique-id-for-html-anchor"
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct RosaryPrayer {
    pub en: String,
    pub de: String,
    pub la: String,
    pub source_de: String,
    pub source_en: String,
    pub source_la: String,
    pub source_link_de: String,
    pub source_link_en: String,
    pub source_link_la: String,
    pub image: String,
}

/// A simple helper to do multiple replacements in a template string.
fn replace_all(mut text: String, pairs: &[(&str, &str)]) -> String {
    for (k, v) in pairs {
        text = text.replace(k, v);
    }
    text
}

/// Render the "Our Father" snippet.
fn render_rosary_ourfather(
    template: &str,
    decade_label: &str,
    section_id: &str,
    prev_id: &str,
    next_id: &str,
) -> String {
    replace_all(
        template.to_string(),
        &[
            ("$$ID$$", section_id),
            ("$$PREV_ID$$", prev_id),
            ("$$NEXT_ID$$", next_id),
            ("$$DECADE$$", decade_label),
        ],
    )
}

/// Render the "Glory Be" snippet.
fn render_rosary_glorybe(
    template: &str,
    decade_label: &str,
    section_id: &str,
    prev_id: &str,
    next_id: &str,
) -> String {
    replace_all(
        template.to_string(),
        &[
            ("$$ID$$", section_id),
            ("$$PREV_ID$$", prev_id),
            ("$$NEXT_ID$$", next_id),
            ("$$DECADE$$", decade_label),
        ],
    )
}

/// Render the "Fatima" prayer snippet.
fn render_rosary_fatima(
    template: &str,
    decade_label: &str,
    section_id: &str,
    prev_id: &str,
    next_id: &str,
) -> String {
    replace_all(
        template.to_string(),
        &[
            ("$$ID$$", section_id),
            ("$$PREV_ID$$", prev_id),
            ("$$NEXT_ID$$", next_id),
            ("$$DECADE$$", decade_label),
        ],
    )
}

/// Render the "nav" snippet that provides some anchor or navigation link.
fn render_nav(template: &str, section_id: &str) -> String {
    replace_all(template.to_string(), &[("$$ID$$", section_id)])
}

/// Render a single Hail Mary block (the "mystery_section_html" template).
///
/// - `decade_label`: the descriptive name of the decade (e.g. "The Annunciation").
/// - `index`: the Hail Mary index (1..10).
/// - `text_top`: the reflection text for this Hail Mary (language-specific).
/// - `source`: e.g. Scripture or other reference
/// - `section_id`: unique ID for the anchor
/// - `prev_id` / `next_id`: IDs for the navigational "previous" and "next" sections
/// - `hm_start`: the first half of the Hail Mary (lang-specific)
/// - `hm_end`: the second half of the Hail Mary (lang-specific)
/// - `hma`: the short addition like “whom you, O Virgin, received from the Holy Spirit.”
///          (for the end of "...the fruit of thy womb, Jesus, {ADDITION} Holy Mary, ...")
fn render_rosary_section(
    lang: &str,
    template: &str,
    decade_label: &str,
    index: usize,
    prayer: &RosaryPrayer,
    section_id: &str,
    prev_id: &str,
    next_id: &str,
    hma: &str,
    meta: &MetaJson,
) -> String {
    let text_top = match lang {
        "de" => &prayer.de,
        "en" => &prayer.en,
        "la" => &prayer.la,
        _ => "",
    };

    let text_top_source = match lang {
        "de" => &prayer.source_de,
        "en" => &prayer.source_en,
        "la" => &prayer.source_la,
        _ => "",
    };

    let text_top_link = match lang {
        "de" => &prayer.source_link_de,
        "en" => &prayer.source_link_en,
        "la" => &prayer.source_link_la,
        _ => "",
    };

    let hm = get_string(meta, lang, "hailmary")
        .unwrap_or_default()
        .replace("$$SPECIAL$$", &format!("<strong><em>{hma}</em></strong>"));

    replace_all(
        template.to_string(),
        &[
            ("$$SECTION_ID$$", section_id),
            ("$$DECADE$$", decade_label),
            ("$$INDEX$$", &index.to_string()),
            ("$$TEXT_TOP$$", &text_top),
            ("$$SOURCE$$", &text_top_source),
            ("$$SOURCE_LINK$$", &text_top_link),
            ("$$PREV_SECTION_ID$$", prev_id),
            ("$$NEXT_SECTION_ID$$", next_id),
            ("$$HAIL_MARY$$", &hm),
        ],
    )
}

/// Returns the language-specific "additional phrase" appended to
/// the Hail Mary ("...Jesus, who/den du, o Jungfrau...")
///
/// This matches the logic from the Python code:
///
/// ```python
/// hm_add = [
///   "den du, o Jungfrau, vom Heiligen Geist empfangen hast.", ...
///   ...
/// ]
/// ```
fn get_hail_mary_addition(meta: &MetaJson, lang: &str, index_of_mystery: usize) -> String {
    get_string(meta, lang, &format!("hm-decade-{index_of_mystery}")).unwrap_or_default()
}

/// This function reproduces the logic of the Python code's `render_rosary_body(...)`.
/// It uses the given RosaryTemplates (for partial HTML fragments) and
/// RosaryMysteries (the parsed "mysteries.json") to build the complete
/// Rosary HTML string.
pub fn generate_rosary(
    lang: &str,
    templates: &RosaryTemplates,
    mysteries_json: &RosaryMysteries,
    meta: &MetaJson,
) -> String {
    // The label for the "intro" or "start" in each language:
    let start_decade_label = get_string(meta, lang, "decade-start").unwrap_or_default();

    // 1) Begin with the main template for the Rosary layout:
    let mut rosary_html = templates.main_html.clone();

    // 2) Fill in the placeholders for the "intro" section
    //    (Our Father, Glory Be, Fatima, and two nav placeholders).
    let intro_ourfather = render_rosary_ourfather(
        &templates.ourfather_html,
        &start_decade_label,
        "intro-05",
        "intro-04",
        "intro-06",
    );
    rosary_html = rosary_html.replace("<!-- OURFATHER -->", &intro_ourfather);

    let intro_glorybe = render_rosary_glorybe(
        &templates.glorybe_html,
        &start_decade_label,
        "intro-09",
        "intro-08",
        "intro-10",
    );
    rosary_html = rosary_html.replace("<!-- GLORYBE -->", &intro_glorybe);

    let intro_fatima = render_rosary_fatima(
        &templates.fatima_html,
        &start_decade_label,
        "intro-10",
        "intro-09",
        "decade-1-ourfather",
    );
    rosary_html = rosary_html.replace("<!-- FATIMA -->", &intro_fatima);

    let intro_nav_01 = render_nav(&templates.nav_html, "intro-00");
    rosary_html = rosary_html.replace("<!-- NAV_01 -->", &intro_nav_01);

    let intro_nav_02 = render_nav(&templates.nav_html, "intro-11");
    rosary_html = rosary_html.replace("<!-- NAV_02 -->", &intro_nav_02);

    // 3) Insert the "outro" at the bottom:
    rosary_html = rosary_html.replace("<!-- END -->", &templates.outro_html);

    // 4) Build up the 15 mysteries (Joyful, Sorrowful, Glorious)
    //    or however many you want to handle:
    let mut mysteries_accum = String::new();

    for i in 1..=15 {
        let i_str = i.to_string();
        if let Some(mystery) = mysteries_json.mysteries.get(&i_str) {
            // Decade label in the chosen language:
            let decade_label = mystery
                .decade
                .get(lang)
                .unwrap_or(&String::new())
                .to_owned();

            // The additional phrase appended to "Jesus," for each Hail Mary:
            let hma = get_hail_mary_addition(meta, lang, i);

            // 4a) Our Father snippet for each decade:
            let ourfather_id = format!("decade-{}-ourfather", i);
            let ourfather_prev = if i == 1 {
                "intro-10".to_string()
            } else {
                format!("decade-{}-fatima", i - 1)
            };
            let ourfather_next = if i == 15 {
                // after the 15th decade, jump to the outro
                "outro-01".to_string()
            } else {
                // or else to the first Hail Mary of the decade
                // typically "s<image-of-first-prayer>"
                // We'll guess "s{prayer.image}" for the first prayer
                // but you can adjust logic as needed.
                if let Some(first_prayer) = mystery.prayers.get("1") {
                    format!("s{}", first_prayer.image)
                } else {
                    // fallback
                    format!("decade-{}-ourfather", i + 1)
                }
            };

            let decade_ourfather = render_rosary_ourfather(
                &templates.ourfather_html,
                &decade_label,
                &ourfather_id,
                &ourfather_prev,
                &ourfather_next,
            );
            mysteries_accum.push_str(&decade_ourfather);

            // 4b) The 10 Hail Mary prayers:
            for q in 1..=10 {
                let q_str = q.to_string();
                if let Some(prayer) = mystery.prayers.get(&q_str) {
                    // anchor IDs for previous and next
                    let section_id = &prayer.image;
                    let prev_section_id = if q == 1 {
                        // the Our Father snippet
                        ourfather_id.clone()
                    } else {
                        // e.g. "s<previous prayer's image>"
                        let prev_index = (q - 1).to_string();
                        if let Some(prev_p) = mystery.prayers.get(&prev_index) {
                            format!("s{}", prev_p.image)
                        } else {
                            // fallback
                            ourfather_id.clone()
                        }
                    };

                    let next_section_id = if q == 10 {
                        // after the 10th Hail Mary: glorybe
                        format!("decade-{}-glorybe", i)
                    } else {
                        // the next Hail Mary
                        let next_index = (q + 1).to_string();
                        if let Some(next_p) = mystery.prayers.get(&next_index) {
                            format!("s{}", next_p.image)
                        } else {
                            // fallback
                            format!("decade-{}-glorybe", i)
                        }
                    };

                    let hail_mary_html = render_rosary_section(
                        lang,
                        &templates.mystery_section_html,
                        &decade_label,
                        q,
                        prayer,
                        section_id,
                        &prev_section_id,
                        &next_section_id,
                        &hma,
                        meta,
                    );
                    mysteries_accum.push_str(&hail_mary_html);
                }
            }

            // 4c) "Glory Be" for this decade
            let glorybe_id = format!("decade-{}-glorybe", i);
            let glorybe_prev = format!(
                "s{}",
                // the 10th Hail Mary
                if let Some(prayer10) = mystery.prayers.get("10") {
                    &prayer10.image
                } else {
                    ""
                }
            );
            let glorybe_next = format!("decade-{}-fatima", i);
            let decade_glorybe = render_rosary_glorybe(
                &templates.glorybe_html,
                &decade_label,
                &glorybe_id,
                &glorybe_prev,
                &glorybe_next,
            );
            mysteries_accum.push_str(&decade_glorybe);

            // 4d) "Fatima" prayer
            let fatima_id = format!("decade-{}-fatima", i);
            let fatima_prev = glorybe_id.clone();
            let fatima_next = if i % 5 == 0 {
                // after each set of 5 mysteries, show the nav
                format!("nav-{}", i)
            } else {
                // otherwise, go to the next decade's Our Father
                format!("decade-{}-ourfather", i + 1)
            };
            let decade_fatima = render_rosary_fatima(
                &templates.fatima_html,
                &decade_label,
                &fatima_id,
                &fatima_prev,
                &fatima_next,
            );
            mysteries_accum.push_str(&decade_fatima);

            // 4e) Possibly insert a "nav" snippet after every 5 mysteries
            if i % 5 == 0 {
                let nav_id = format!("nav-{}", i);
                let nav_html = render_nav(&templates.nav_html, &nav_id);
                mysteries_accum.push_str(&nav_html);
            }
        }
    }

    // 5) Finally, place the entire block of 15 mysteries into the <!-- MYSTERIES --> placeholder.
    rosary_html = rosary_html.replace("<!-- MYSTERIES -->", &mysteries_accum);

    rosary_html
}
