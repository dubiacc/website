use std::collections::HashMap;
use serde::Deserialize;

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
    pub source: String,
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
    next_id: &str
) -> String {
    replace_all(template.to_string(), &[
        ("$$ID$$", section_id),
        ("$$PREV_ID$$", prev_id),
        ("$$NEXT_ID$$", next_id),
        ("$$DECADE$$", decade_label),
    ])
}

/// Render the "Glory Be" snippet.
fn render_rosary_glorybe(
    template: &str,
    decade_label: &str,
    section_id: &str,
    prev_id: &str,
    next_id: &str
) -> String {
    replace_all(template.to_string(), &[
        ("$$ID$$", section_id),
        ("$$PREV_ID$$", prev_id),
        ("$$NEXT_ID$$", next_id),
        ("$$DECADE$$", decade_label),
    ])
}

/// Render the "Fatima" prayer snippet.
fn render_rosary_fatima(
    template: &str,
    decade_label: &str,
    section_id: &str,
    prev_id: &str,
    next_id: &str
) -> String {
    replace_all(template.to_string(), &[
        ("$$ID$$", section_id),
        ("$$PREV_ID$$", prev_id),
        ("$$NEXT_ID$$", next_id),
        ("$$DECADE$$", decade_label),
    ])
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
    template: &str,
    decade_label: &str,
    index: usize,
    text_top: &str,
    source: &str,
    section_id: &str,
    prev_id: &str,
    next_id: &str,
    hm_start: &str,
    hm_end: &str,
    hma: &str,
) -> String {
    replace_all(template.to_string(), &[
        ("$$SECTION_ID$$", section_id),
        ("$$DECADE$$", decade_label),
        ("$$INDEX$$", &index.to_string()),
        ("$$TEXT_TOP$$", text_top),
        ("$$SOURCE$$", source),
        ("$$PREV_SECTION_ID$$", prev_id),
        ("$$NEXT_SECTION_ID$$", next_id),
        ("$$HAIL_MARY_START$$", hm_start),
        ("$$DECADE_ADDITION$$", hma),
        ("$$HAIL_MARY_END$$", hm_end),
    ])
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
fn get_hail_mary_addition(lang: &str, index_of_mystery: usize) -> &'static str {
    // For the sake of example, we only show 15 additions.
    // You can expand or modify as needed.
    static DE: [&str; 15] = [
        "den du, o Jungfrau, vom Heiligen Geist empfangen hast.",
        "den du, o Jungfrau, zu Elisabeth getragen hast.",
        "den du, o Jungfrau, in Bethlehem geboren hast.",
        "den du, o Jungfrau, im Tempel aufgeopfert hast.",
        "den du, o Jungfrau, im Tempel wiedergefunden hast.",
        "der für uns Blut geschwitzt hat.",
        "der für uns gegeißelt worden ist.",
        "der für uns mit Dornen gekrönt worden ist.",
        "der für uns das schwere Kreuz getragen hat.",
        "der für uns am Kreuz gestorben ist.",
        "der von den Toten auferstanden ist.",
        "der in den Himmel aufgefahren ist.",
        "der uns den Heiligen Geist gesandt hat.",
        "der dich, o Jungfrau, in den Himmel aufgenommen hat.",
        "der dich, o Jungfrau, im Himmel gekrönt hat.",
    ];

    static EN: [&str; 15] = [
        "whom you, O Virgin, received from the Holy Spirit.",
        "whom you, O Virgin, carried to Elizabeth.",
        "to whom, O Virgin, you gave birth in Bethlehem.",
        "whom you, O Virgin, offered up in the temple.",
        "whom you, O Virgin, found again in the temple.",
        "who sweated blood for us.",
        "who was scourged for us.",
        "who was crowned with thorns for us.",
        "who bore the heavy cross for us.",
        "who died for us on the cross.",
        "who rose from the dead.",
        "who ascended into heaven.",
        "who sent us the Holy Spirit.",
        "who took you, O Virgin, up into heaven.",
        "who crowned you, O Virgin, in heaven.",
    ];

    // index_of_mystery is 1..=15 in the code:
    let idx = (index_of_mystery - 1).min(14);
    match lang {
        "de" => DE[idx],
        _     => EN[idx],
    }
}

/// This function reproduces the logic of the Python code's `render_rosary_body(...)`.
/// It uses the given RosaryTemplates (for partial HTML fragments) and 
/// RosaryMysteries (the parsed "mysteries.json") to build the complete 
/// Rosary HTML string.
pub fn generate_rosary(
    lang: &str,
    templates: &RosaryTemplates,
    mysteries_json: &RosaryMysteries,
) -> String {
    // Language-specific lines for the Hail Mary:
    let (hm_start, hm_end) = match lang {
        "de" => (
            "Gegrüßet seiest du Maria, voll der Gnaden, der Herr ist mit dir. \
             Du bist gebenedeit unter den Weibern und gebenedeit ist die Frucht \
             deines Leibes Jesus, ",
            " Heilige Maria, Mutter Gottes, bitte für uns Sünder, \
             jetzt und in der Stunde unseres Todes.",
        ),
        _ => (
            "Hail, Mary, full of grace, the Lord is with thee. \
             Blessed art thou amongst women and blessed is the fruit \
             of thy womb, Jesus, ",
            " Holy Mary, Mother of God, pray for us sinners, \
             now and at the hour of our death.",
        ),
    };

    // The label for the "intro" or "start" in each language:
    let start_decade_label = if lang == "de" { "Anfang" } else { "Start" };

    // 1) Begin with the main template for the Rosary layout:
    let mut rosary_html = templates.main_html.clone();

    // 2) Fill in the placeholders for the "intro" section
    //    (Our Father, Glory Be, Fatima, and two nav placeholders).
    let intro_ourfather = render_rosary_ourfather(
        &templates.ourfather_html, 
        start_decade_label, 
        "intro-05", 
        "intro-04", 
        "intro-06"
    );
    rosary_html = rosary_html.replace("<!-- OURFATHER -->", &intro_ourfather);

    let intro_glorybe = render_rosary_glorybe(
        &templates.glorybe_html, 
        start_decade_label, 
        "intro-09", 
        "intro-08", 
        "intro-10"
    );
    rosary_html = rosary_html.replace("<!-- GLORYBE -->", &intro_glorybe);

    let intro_fatima = render_rosary_fatima(
        &templates.fatima_html, 
        start_decade_label, 
        "intro-10", 
        "intro-09", 
        "decade-1-ourfather"
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
            let hma = get_hail_mary_addition(lang, i);

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
                &ourfather_next
            );
            mysteries_accum.push_str(&decade_ourfather);

            // 4b) The 10 Hail Mary prayers:
            for q in 1..=10 {
                let q_str = q.to_string();
                if let Some(prayer) = mystery.prayers.get(&q_str) {
                    // Choose the correct text based on `lang`:
                    let hail_mary_reflection = if lang == "de" {
                        &prayer.de
                    } else {
                        &prayer.en
                    };

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
                        &templates.mystery_section_html,
                        &decade_label,
                        q,
                        hail_mary_reflection,
                        &prayer.source,
                        section_id,
                        &prev_section_id,
                        &next_section_id,
                        hm_start,
                        hm_end,
                        hma,
                    );
                    mysteries_accum.push_str(&hail_mary_html);
                }
            }

            // 4c) "Glory Be" for this decade
            let glorybe_id = format!("decade-{}-glorybe", i);
            let glorybe_prev = format!("s{}", 
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
                &glorybe_next
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
                &fatima_next
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
