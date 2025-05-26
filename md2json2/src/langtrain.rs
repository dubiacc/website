//! Generate "language trainer" pages.

use serde::{Deserialize, Serialize};

use crate::MetaJson;

static LANGTRAIN_CSS: &str = include_str!("../../templates/latin.css");
static LANGTRAIN_JS: &str = include_str!("../../templates/latin.grammar.js");
static LESSON_HTML: &str = include_str!("../../templates/latin.lesson.html");

pub enum TrainLang {
    Latin,
}

impl TrainLang {
    pub fn get_initial_vocab(&self, lang: &str) -> Vec<VocabCsvRow> {
        match lang {
            "en" => parse_vocab_csv(include_str!(
                "../../templates/latin/latin.vocab.1.csv"
            ))
            .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    pub fn get_grammar_lessons(&self, lang: &str) -> GrammarLessons {
        match lang {
            "en" => parse_grammar_lessons(include_str!(
                "../../templates/latin/latin.grammar.json"
            ))
            .unwrap_or_default(),
            _ => GrammarLessons::default(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GrammarLessons {
    pub sections: Vec<GrammarLesson>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GrammarLesson {
    pub id: usize,
    pub title: String,
    pub lesson: Vec<String>,
    pub help_section: Vec<String>,
    pub tests: Vec<GrammarTest>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GrammarTest {
    pub id: String,
    pub sentences: Vec<GrammarTestSentence>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GrammarTestSentence {
    pub segments: Vec<GrammarTestSentenceItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GrammarTestSentenceItem {
    S(String),
    I(GrammarTestSentencePlaceholder),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GrammarTestSentencePlaceholder {
    #[serde(rename = "placeholderId")]
    pub placeholder_id: usize,
    #[serde(rename = "baseForm")]
    pub base_form: String,
    pub answers: Vec<String>,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VocabCsvRow {
    pub userlang: String,
    pub targetlang: String,
    pub description: Option<String>,
    pub example_targetlang: Option<String>,
    pub example_sourcelang: Option<String>,
    pub popularity: Option<usize>,
}

fn parse_grammar_lessons(s: &str) -> Option<GrammarLessons> {
    serde_json::from_str(s).ok()
}

fn parse_vocab_csv(input: &str) -> Option<Vec<VocabCsvRow>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(input.as_bytes());

    let mut vocab_rows = Vec::new();

    for result in rdr.records() {
        let record = result.ok()?;
        let row = VocabCsvRow {
            userlang: record.get(0).unwrap().to_string(),
            targetlang: record.get(1).unwrap().to_string(),
            description: Some(record.get(2).unwrap().to_string()),
            example_targetlang: None,
            example_sourcelang: Some(record.get(3).unwrap().to_string()),
            popularity: Some(record.get(4).unwrap().parse().ok()?),
        };
        vocab_rows.push(row);
    }

    Some(vocab_rows)
}

pub fn generate_langtrain_content(
    lang: &str,
    train_lang: TrainLang,
    meta: &MetaJson,
) -> Result<String, String> {
    let mut content = String::new();
    content.push_str(&format!("<style>{LANGTRAIN_CSS}</style>"));

    let initial_vocab = train_lang.get_initial_vocab(lang);
    let grammar_lessons = train_lang.get_grammar_lessons(lang);

    // TODO: render vocab test script

    /*
        <div id="progressBarContainer">
            <div id="progressBar"></div>
        </div>

        <div id="vocabContainer"></div>

        <button id="takeTestBtn" class="button">Take vocabulary test</button>
        <button id="checkResultsBtn" class="button hidden">Check results</button>
        <div id="resultsSummary" class="hidden"></div>
        <button id="nextLessonBtn" class="button hidden">Next lesson</button>
    */

    let submit_test = crate::get_string(meta, lang, "grammar-test-submit")?;
    let reload_test = crate::get_string(meta, lang, "grammar-test-reload")?;

    for lesson in grammar_lessons.sections.iter() {
        let mut les = LESSON_HTML.to_string();
        les = les.replace("$$SUBMIT_TEST$$", &submit_test);
        les = les.replace("$$RELOAD_TEST$$", &reload_test);
        les = les.replace("$$TITLE$$", &lesson.title);
        les = les.replace("$$LESSON_ID$$", &lesson.id.to_string());
        les = les.replace("<!-- LESSON_CONTENT -->", &lesson.lesson.join("\r\n"));
        les = les.replace("<!-- HELP_CONTENT -->", &lesson.help_section.join("\r\n"));
        content.push_str(&les);
    }

    // TODO: Generate lesson test script
    let repl = format!(
        "const courseData = JSON.parse(atob('{}'));",
        base64::encode(&serde_json::to_string_pretty(&grammar_lessons).unwrap_or_default())
    );
    let langtrain_js = LANGTRAIN_JS
        .replace("const courseData = {};", &repl)
        .replace(
            "COURSE_DATA_LEN",
            &grammar_lessons.sections.len().to_string(),
        );
    content.push_str(&format!("<script>{langtrain_js}</script>"));

    Ok(content)
}
