use std::path::Path;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use collection_macros::btreemap;
use serde_derive::{Serialize, Deserialize};

#[derive(Debug, Default)]
struct LoadedArticles {
    langs: BTreeMap<Lang, BTreeMap<Slug, String>>,
}

#[derive(Debug, Clone, Copy)]
enum ArticleType {
    Question,
    Tract,
    Prayer,
}

#[derive(Debug)]
struct VectorizedArticle {
    words: Vec<usize>,
    atype: ArticleType,
    parsed: ParsedArticle,
}

#[derive(Debug, Default)]
struct VectorizedArticles {
    map: BTreeMap<Lang, BTreeMap<Slug, VectorizedArticle>>,
}

#[derive(Debug, Default)]
struct AnalyzedArticles {
    map: BTreeMap<Lang, BTreeMap<Slug, ParsedArticleAnalyzed>>,
}

#[derive(Debug, Default)]
struct ParsedArticle {
    title: String,
    date: String,
    tags: Vec<String>,
    authors: Vec<String>,
    sha256: String,
    img: Option<Image>,
    summary: Vec<Paragraph>,
    article_abstract: Vec<Paragraph>,
    sections: Vec<ArticleSection>,
    footnotes: Vec<String>,
}

impl VectorizedArticles {
    pub fn analyze(&self) -> AnalyzedArticles {
        AnalyzedArticles {
            map: self.map.iter().map(|(lang, v)| {
                (lang.clone(), v.iter().map(|(slug, vectorized)| {
                    let similar = get_similar_articles(vectorized, slug, v);
                    (slug.clone(), ParsedArticleAnalyzed {
                        title: vectorized.parsed.title.clone(),
                        date: vectorized.parsed.date.clone(),
                        tags: vectorized.parsed.tags.clone(),
                        authors: vectorized.parsed.authors.clone(),
                        sha256: vectorized.parsed.sha256.clone(),
                        img: vectorized.parsed.img.clone(),
                        summary: vectorized.parsed.summary.clone(),
                        sections: vectorized.parsed.sections.clone(),
                        related: similar,
                        article_abstract: vectorized.parsed.article_abstract.clone(),
                        footnotes: vectorized.parsed.footnotes.clone(),
                    })
                }).collect())
            }).collect()
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct ParsedArticleAnalyzed {
    title: String,
    date: String,
    tags: Vec<String>,
    authors: Vec<String>,
    sha256: String,
    img: Option<Image>,
    summary: Vec<Paragraph>,
    article_abstract: Vec<Paragraph>,
    sections: Vec<ArticleSection>,
    related: Vec<String>,
    footnotes: Vec<String>, // BTreeMap<String, Paragraph>,
}

impl ParsedArticleAnalyzed {
    pub fn is_prayer(&self) -> bool {
        self.tags.iter().any(|s| s == "gebet" || s == "prayer")
    }
    pub fn get_date(&self) -> Option<(&str, &str, &str)> {
        let mut iter = self.date.split("-");
        let year = iter.next()?;
        let month = iter.next()?;
        let day = iter.next()?;
        Some((year, month, day))
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct Config {
    #[serde(default)]
    date: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    authors: Vec<String>,
}


#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct ArticleSection {
    title: String,
    indent: usize,
    pars: Vec<Paragraph>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "d", rename_all = "lowercase")]
enum Paragraph {
    Sentence { s: Vec<SentenceItem> },
    Quote { q: Quote },
    Image { i: Image }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
struct Sentence {
    items: Vec<SentenceItem>
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "d", rename_all = "lowercase")]
enum SentenceItem {
    Text {
        text: String,
    },
    Link {
        l: Link,
    },
    Footnote {
        id: String,
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
enum LinkType {
    Wikipedia,
    Internal,
    Other,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
struct Quote {
    title: String,
    quote: Vec<String>,
    author: String,
    author_link: String,
    source: String,
    source_link: String,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
struct Link {
    text: String,
    href: String,
}

impl Link {
    pub fn new(s: &str) -> Option<Self> {

        let mut s = s.trim().to_string();
        if s.starts_with("[") {
            s = s.replacen("[", "", 1);
        } else {
            return None;
        }

        let iter = s.split("](").collect::<Vec<_>>();
        let alt = iter.get(0)?.to_string();
        let mut rest = iter.get(1)?.to_string();
        if rest.ends_with(")") {
            rest = rest.split(")").next()?.to_string();
        } else {
            return None;
        }

        let href = rest.split_whitespace().nth(0)?.to_string();
        let title = rest.split_whitespace().nth(1)
            .map(|s| s.trim().replace("\"", "").replace("'", "").replace("`", ""))
            .unwrap_or(alt.clone());
    
        Some(Self {
            href,
            text: title,
        })
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
struct Image {
    href: String, 
    alt: String,
    title: String,
    inline: bool,
}

fn parse_paragraph(s: &str) -> Paragraph {
    if let Some(i) = Image::new(s.trim()) {
        Paragraph::Image { i }
    } else if let Some(q) = Quote::new(s.trim()) {
        Paragraph::Quote { q }
    } else {
        Paragraph::Sentence {
            s: Sentence::new(s.trim()).items,
        }
    }
}

fn parse_paragraphs(s: &str) -> Vec<Paragraph> {
    let lines = s.lines().map(|q| q.trim()).collect::<Vec<_>>();
    lines.split(|s| s.is_empty())
    .map(|q| q.to_vec())
    .collect::<Vec<Vec<_>>>()
    .iter()
    .filter(|s| !s.is_empty())
    .map(|sp| parse_paragraph(&sp.join("\r\n")))
    .collect()
}

#[cfg(feature = "external")]
fn sha256(s: &str) -> String {
    use sha2::{Sha256, Digest};
    use base64::Engine;
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let result = hasher.finalize();
    base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(result)
}

fn gather_footnotes(l: &[&str]) -> (Vec<String>, BTreeSet<usize>) {
    let mut to_ignore = BTreeSet::new();
    let mut target = Vec::new();
    for (i, l) in l.iter().enumerate() {
        if l.trim().starts_with("[^") && l.contains("]:") {
            to_ignore.insert(i);
            target.push(l.to_string());
        }
    }
    (target, to_ignore)
}

fn extract_config(l: &[&str]) -> (Config, BTreeSet<usize>) {
    let mut codeblock = Vec::new();
    let mut to_ignore = BTreeSet::new();
    let mut in_cb = false;
    for (i, l) in l.iter().enumerate() {
        if l.contains("```") {
            if in_cb {
                in_cb = false;
                to_ignore.insert(i);
            } else {
                in_cb = codeblock.is_empty();
                if in_cb {
                    to_ignore.insert(i);
                }
            }
        } else if in_cb {
            codeblock.push(l.trim());
            to_ignore.insert(i);
        }
    }

    let config = serde_json::from_str::<Config>(
        &codeblock.join("\r\n")
    ).unwrap_or_default();

    (config, to_ignore)
}

fn parse_article(s: &str) -> ParsedArticle {
    
    let lines = s.lines().collect::<Vec<_>>();
    let (title_line, title) = lines.iter().enumerate()
        .filter(|(_, s)| s.starts_with("# "))
        .map(|(i, q)| (i, q.replace("# ", "").trim().to_string()))
        .next()
        .unwrap_or((0, String::new()));

    let sha256 = sha256(&s);

    let (config, lines_to_ignore) = extract_config(&lines);

    let lines_before_heading = lines
        .iter().enumerate()
        .filter_map(|(i, l)| if lines_to_ignore.contains(&i) || i >= title_line { None } else { Some(*l) })
        .collect::<Vec<_>>();

    let lines_after_heading = lines
        .iter().enumerate()
        .filter_map(|(i, l)| if lines_to_ignore.contains(&i) || i <= title_line { None } else { Some(*l) })
        .collect::<Vec<_>>();

    let article_abstract = lines_after_heading
        .iter()
        .take_while(|s| !s.contains("# "))
        .cloned()
        .collect::<Vec<_>>();

    let lines_after_heading = lines_after_heading[article_abstract.len()..].to_vec();
    let (footnotes, footnote_lines) = gather_footnotes(&lines_after_heading);
    let lines_after_heading = lines_after_heading.iter().enumerate().filter_map(|(i, s)| {
        if footnote_lines.contains(&i) {
            None
        } else {
            Some(s)
        }
    }).collect::<Vec<_>>();

    let mut sections = lines_after_heading
    .iter().enumerate()
    .filter_map(|(i, s)| {
        if s.contains("# ") {
            Some(i)
        } else {
            None
        }
    }).collect::<Vec<_>>();
    sections.push(lines_after_heading.len());

    let sections = sections.windows(2).filter_map(|s| {
        
        let (start_line, end_line) = match s {
            [s, e] => (*s, *e),
            _ => return None,
        };

        let l = lines_after_heading.get(start_line)?;
        let indent = l.chars().filter(|c| *c == '#').count();
        let title = l.replace("#", "").trim().to_string();

        let lines = ((start_line + 1)..end_line)
            .filter_map(|i| lines_after_heading.get(i))
            .map(|s| **s)
            .collect::<Vec<_>>();

        let pars = parse_paragraphs(&lines.join("\r\n"));

        Some(ArticleSection {
            title,
            indent,
            pars,
        })
    }).collect::<Vec<_>>();

    ParsedArticle {
        title,
        date: config.date,
        tags: config.tags,
        authors: config.authors,
        sha256: sha256,
        img: None,
        summary: parse_paragraphs(&lines_before_heading.join("\r\n")),
        article_abstract: parse_paragraphs(&article_abstract.join("\r\n")),
        sections,
        footnotes,
    }
}

impl ArticleType {
    /// Returns the type of article based on text content heuristics
    pub fn new(s: &str) -> ArticleType {
        let is_question = s.lines().filter(|q| q.starts_with("# ")).any(|q| q.trim().ends_with("?"));
        let is_prayer = s.lines().any(|q| q.trim() == "Amen.") || s.lines().any(|s| s.contains("tags") && (s.contains("gebet") || s.contains("prayer")));
        if is_prayer {
            ArticleType::Prayer
        } else if is_question {
            ArticleType::Question
        } else {
            ArticleType::Tract
        }
    }
}

impl Quote {
    fn new(s: &str) -> Option<Self> {

        let mut lines = s.trim().lines()
            .map(|l| l.trim())
            .filter(|l| l.trim().starts_with(">"))
            .map(|l| l.replacen(">", "", 1).trim().to_string())
            .collect::<Vec<_>>();

        if lines.is_empty() {
            return None;
        }

        let title = lines.iter()
            .find(|s| s.starts_with("**"))
            .cloned();

        if let Some(t) = title.as_deref() {
            lines.retain(|l| l.as_str() != t);
        }

        let title = title
            .map(|l| l.replace("**", ""))
            .unwrap_or_default();

        let author_line = lines.iter()
            .find(|s| s.trim().starts_with("--") || s.trim().starts_with("—-"))
            .cloned();
        
        if let Some(t) = author_line.as_deref() {
            lines.retain(|l| l.as_str() != t);
        }
        
        let author_line = author_line
            .map(|s| s.replacen("--", "", 1).replacen("—-", "", 1).trim().to_string())
            .unwrap_or_default();

        let mut author = String::new();
        let mut author_link = String::new();
        let mut source = String::new();
        let mut source_link = String::new();

        let mut author_line = &author_line[..];

        if let Some((next_link, to_delete)) = take_next_link(&author_line) {
            author = next_link.text;
            author_link = next_link.href;
            author_line = &author_line[to_delete..];
        }

        let next_link_start = author_line
        .char_indices()
        .find_map(|(idx, c)| if c == '[' { Some(idx) } else { None });

        if let Some(nls) = next_link_start {
            author_line = &author_line[nls..];
        }

        if let Some((next_link, _)) = take_next_link(&author_line) {
            source = next_link.text;
            source_link = next_link.href;
        }

        let lines = lines.iter()
            .filter(|s| !s.starts_with("**"))
            .filter(|s| !s.starts_with("--"))
            .cloned()
            .collect::<Vec<_>>();

        let mut quote = lines
            .split(|s| s.trim().is_empty())
            .map(|q| q.join(" "))
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<String>>();

        if let Some(fl) = quote.first_mut() {
            if fl.trim().starts_with("\"") {
                *fl = fl.replacen("\"", "", 1);
            } else if fl.trim().starts_with("'") {
                *fl = fl.replacen("'", "", 1);
            } else if fl.trim().starts_with("`") {
                *fl = fl.replacen("`", "", 1);
            } 
        }

        if let Some(fl) = quote.last_mut() {
            if fl.trim().ends_with("\"") {
                *fl = fl.replacen("\"", "", 1);
            } else if fl.trim().ends_with("'") {
                *fl = fl.replacen("'", "", 1);
            } else if fl.trim().ends_with("`") {
                *fl = fl.replacen("`", "", 1);
            } 
        }

        let q = Quote {
            title,
            quote,
            author,
            author_link,
            source,
            source_link,
        };

        Some(q)
    }
}

// Given a string, returns the extracted link + number of bytes to be consumed
fn take_next_link(s: &str) -> Option<(Link, usize)> {

    if !s.trim().starts_with("[") {
        return None;
    }

    let end = s.char_indices()
    .find_map(|(id, ch)| {
        if ch == ')' { Some(id) } else { None }
    })?;

    let substring = &s[..(end + 1)];

    Link::new(substring).map(|q| (q, end + 1))
}

#[test]
fn test_quote_2() {
    let s = "
        > Wenn ein Mann eine Jungfrau trifft, die nicht verlobt ist
        > 
        > —- [5. Mose 22,28-29](https://k-bibel.de/ARN/Deuteronomium22#28-29)
    ";

    assert_eq!(Quote::new(s), Some(Quote {
        title: "Heading".to_string(),
        quote: vec![
            "Wenn ein Mann eine Jungfrau trifft, die nicht verlobt ist".to_string(), 
        ],
        author: "5. Mose 22,28-29".to_string(),
        author_link: "https://k-bibel.de/ARN/Deuteronomium22#28-29".to_string(),
        source: "".to_string(),
        source_link: "".to_string(),
    }))
}
#[test]
fn test_quote() {

    let s = "
        > **Heading**
        >
        > LineA
        > LineB
        > LineC
        >
        > LineD
        > LineE
        >
        > -- [Test](https://wikipedia.org/Test): [de juiribus](test.pdf)
    ";

    assert_eq!(Quote::new(s), Some(Quote {
        title: "Heading".to_string(),
        quote: vec![
            "LineA LineB LineC".to_string(), 
            "LineD LineE".to_string()
        ],
        author: "Test".to_string(),
        author_link: "https://wikipedia.org/Test".to_string(),
        source: "de juiribus".to_string(),
        source_link: "test.pdf".to_string(),
    }))
}

// parses the footnote from a "[^note]" text
fn parse_footnote_maintext(s: &str) -> Option<(String, usize)> {
    
    if !s.trim().starts_with("[^") {
        return None;
    }

    let end = s.char_indices().find_map(|(idx, c)| {
        if c == ']' {
            Some(idx)
        } else {
            None
        }
    })?;

    let substring = s[2..end].to_string();
    Some((substring, end + 1))
}

impl Sentence {
    fn new(s: &str) -> Self {

        let mut items = Vec::new();
        let mut cur_sentence = Vec::new();
        let mut iter = s.char_indices().peekable();

        while let Some((idx, c)) = iter.next() {
            let next = iter.peek();
            match (c, next.map(|q| q.1)) {
                ('[', Some('^')) => match parse_footnote_maintext(&s[idx..]) {
                    Some((footnote_id, chars_to_skip)) => {
                        if !cur_sentence.is_empty() {
                            items.push(SentenceItem::Text { 
                                text: cur_sentence.iter().cloned().collect::<String>().lines().collect::<Vec<_>>().join(" ") 
                            });
                        }
                        items.push(SentenceItem::Footnote { id: footnote_id });
                        cur_sentence.clear();
                        for _ in 0..chars_to_skip.saturating_sub(1) {
                            let _ = iter.next();
                        }
                    },
                    None => {
                        cur_sentence.push(c);
                    }
                },
                ('[', _) => match take_next_link(&s[idx..]) {
                    Some((link, chars_to_skip)) => {
                        if !cur_sentence.is_empty() {
                            items.push(SentenceItem::Text { 
                                text: cur_sentence.iter().cloned().collect::<String>().lines().collect::<Vec<_>>().join(" ") 
                            });
                        }
                        items.push(SentenceItem::Link { l: link });
                        cur_sentence.clear();
                        for _ in 0..chars_to_skip.saturating_sub(1) {
                            let _ = iter.next();
                        }
                    },
                    None => {
                        cur_sentence.push(c);
                    }
                },
                _ => { cur_sentence.push(c); },
            }
        }

        if !cur_sentence.is_empty() {
            items.push(SentenceItem::Text { 
                text: cur_sentence.iter().cloned().collect::<String>().lines().collect::<Vec<_>>().join(" ") 
            });
        }

        Self { items }
    }
}

#[test]
fn test_sentence() {
    let s = "This is a sentence with a footnote[^15] and a [link](url).";
    assert_eq!(Sentence::new(s), Sentence {
        items: vec![
            SentenceItem::Text { text: "This is a sentence with a footnote".to_string() },
            SentenceItem::Footnote { id: "15".to_string() },
            SentenceItem::Text { text: " and a ".to_string() },
            SentenceItem::Link { l: Link { text: "link".to_string(), href: "url".to_string() } },
            SentenceItem::Text { text: ".".to_string() },
        ]
    })
}

impl Image {
    pub fn new(s: &str) -> Option<Self> {

        let mut s = s.trim().to_string();
        if s.starts_with("![") {
            s = s.replacen("![", "", 1);
        } else {
            return None;
        }

        let iter = s.split("](").collect::<Vec<_>>();
        let alt = iter.get(0)?.to_string();
        let mut rest = iter.get(1)?.to_string();
        if rest.ends_with(")") {
            rest = rest.split(")").next()?.to_string();
        } else {
            return None;
        }

        let href = rest.split_whitespace().nth(0)?.to_string();
        let title = rest.split_whitespace().nth(1)
            .map(|s| s.trim().replace("\"", "").replace("'", "").replace("`", ""))
            .unwrap_or(alt.clone());

        let inline = alt.contains(" :: inline");
        let alt = alt.replace(" :: inline", "").trim().to_string();

        Some(Self {
            href,
            title,
            alt,
            inline,
        })
    }
}

#[test]
fn test_image() {
    let s = "![alt text](Isolated.png \"Title\")";
    assert_eq!(Image::new(s), Some(Image {
        href: "Isolated.png".to_string(),
        alt: "alt text".to_string(),
        title: "Title".to_string(),
        inline: false,
    }));

    let s = "![alt text](Isolated.png)";
    assert_eq!(Image::new(s), Some(Image {
        href: "Isolated.png".to_string(),
        alt: "alt text".to_string(),
        title: "alt text".to_string(),
        inline: false,
    }));

    let s = "![Test)";
    assert_eq!(Image::new(s), None);
}

impl LoadedArticles {
    pub fn vectorize(&self) -> VectorizedArticles {
    
        fn get_words_of_article(s: &str) -> Vec<&str> {
            s.split_whitespace()
            .filter_map(|s| if s.contains("[") || s.contains("]") || s.len() < 3 { None } else { Some(s) })
            .collect()
        }

        VectorizedArticles {
            map: self.langs.iter().map(|(k, v)| {

                let all_words = v.values().flat_map(|c| get_words_of_article(c)).collect::<BTreeSet<_>>();
                let all_words_indexed = all_words.iter().enumerate().map(|(i, s)| (*s, i)).collect::<BTreeMap<_, _>>();

                (k.clone(), v.iter().map(|(k, v2)| {

                    let embedding = get_words_of_article(v2)
                    .into_iter()
                    .filter_map(|q| all_words_indexed.get(q).copied()).collect();
                    
                    let atype = ArticleType::new(v2);
                    
                    (k.clone(), VectorizedArticle {
                        words: embedding,
                        atype: atype,
                        parsed: parse_article(v2),
                    })
                }).collect())
            }).collect()
        }
    }
}

/// return similar articles based on string distance for article N
#[cfg(feature = "external")]
fn get_similar_articles(
    s: &VectorizedArticle, 
    id: &str, 
    map: &BTreeMap<String, VectorizedArticle>
) -> Vec<String> {
    
    let (article_src, article_type) = (&s.words, s.atype);

    let mut target = Vec::new();
    for (other_key, other) in map.iter() {
        
        if other_key == id {
            continue;
        }

        let penalty = match (article_type, other.atype) {
            (ArticleType::Prayer, ArticleType::Prayer) |
            (ArticleType::Tract, ArticleType::Tract) |
            (ArticleType::Question, ArticleType::Question)  => 0,

            (ArticleType::Prayer, _) | 
            (_, ArticleType::Prayer) => continue,
            
            _ => 10000,
        };

        let dst = strsim::generic_damerau_levenshtein(
            article_src, 
            &other.words
        ) + penalty;

        target.push((dst, other_key));
    }

    target.sort_by(|a, b| ((a.0) as usize).cmp(&((b.0) as usize)));
    
    target.into_iter().take(10).map(|s| s.1.clone()).collect()
}

#[cfg(feature = "external")]
fn load_articles(dir: &Path) -> Result<LoadedArticles, String> {

    let entries = 
        walkdir::WalkDir::new(dir)
        .max_depth(5)
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.map_err(|e| e.to_string()).ok()?;
            let entry = entry.path();
            if entry.file_name().and_then(|s| s.to_str()) == Some("index.md") {
                let name = entry.parent()?;
                let lang = name.parent()?;
                let contents = std::fs::read_to_string(&entry).ok()?;

                Some((lang.file_name()?.to_str()?.to_string(), name.file_name()?.to_str()?.to_string(), contents))
            } else {
                None
            }
        }).collect::<Vec<_>>();

    let mut langs = BTreeMap::new();
    for (lang, id, contents) in entries {
        langs.entry(lang)
        .or_insert_with(|| BTreeMap::default())
        .insert(id, contents);
    }

    Ok(LoadedArticles { langs })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct SectionLink {
    slug: String,
    title: String,
}

type Lang = String;
type Slug = String;
type Tag = String;
type Year = String;
type Month = String;
type Day = String;

// type Articles = BTreeMap<Lang, BTreeMap<Slug, VectorizedArticle>>;
type ArticlesByTag = BTreeMap<Lang, BTreeMap<Tag, Vec<SectionLink>>>;
type ArticlesByDate = BTreeMap<Lang, BTreeMap<Year, BTreeMap<Month, BTreeMap<Day, Vec<SectionLink>>>>>;

fn is_prod() -> bool {
    std::env::args().any(|a| a.contains("production"))
}

fn get_root_href() -> &'static str {
    if is_prod() {
        "https://dubia.cc"
    } else {
        "http://127.0.0.1:8080"
    }
}

fn generate_gitignore(articles: &LoadedArticles) -> String {
    let mut filenames = BTreeSet::new();
    for lang in articles.langs.keys() {
        filenames.insert(format!("/{lang}"));
        filenames.insert(format!("/{lang}2"));
        filenames.insert(format!("{lang}.html"));
    }
    filenames.insert("/venv".into());
    filenames.insert("*.md.json".into());
    filenames.insert("md2json-bin".into());
    filenames.insert("index.json".into());
    filenames.insert("index.html".into());
    filenames.insert(".DS_Store".into());
    filenames.insert("/md2json/target".into());
    filenames.insert("/md2json2/target".into());
    filenames.insert("/img2avif/target".into());
    filenames.insert("/md2json/out.txt".into());
    filenames.insert("/venv/*".into());
    return filenames.into_iter().collect::<Vec<_>>().join("\r\n");
}

fn get_title(lang: &str, a: &ParsedArticleAnalyzed) -> String {
    if !a.title.trim().is_empty() {
        return a.title.clone();
    }
    match lang {
        "de" => "Entdecke den wahren katholischen Glauben - dubia.cc".into(),
        "en" => "Discover the true Catholic Faith - dubia.cc".into(),
        _ => String::new()
    }
}

fn si2text(si: &[SentenceItem]) -> String {
    si.iter().map(|s| match s {
        SentenceItem::Footnote { .. } => String::new(),
        SentenceItem::Link { l } => l.text.clone(),
        SentenceItem::Text { text } => text.clone(),
    }).collect::<Vec<_>>().join("")
}

fn par2text(p: &Paragraph) -> String {
    match p {
        Paragraph::Sentence { s } => return si2text(s),
        _ => String::new(),
    }
}

// Returns the description for the <head> tag
fn get_description(lang: &str, a: &ParsedArticleAnalyzed) -> String {
    let try1 = a.summary.get(0).map(|s| par2text(s)).unwrap_or_default();
    if !try1.trim().is_empty() {
        return try1.trim().to_string();
    }
    let try1 = a.article_abstract.get(0).map(par2text).unwrap_or_default();
    if !try1.trim().is_empty() {
        return try1.trim().to_string();
    }
    let sec1 = a.sections.get(0).and_then(|s| s.pars.get(0)).map(par2text).unwrap_or_default();
    if !sec1.trim().is_empty() {
        return sec1.trim().to_string();
    }
    match lang {
        "en" => "dubia is a collection of articles about the traditional Catholic faith. Did the Church really teach...? Answers to errors and questions, prayers, and more!",
        "de" => "dubia ist eine Sammlung von Artikeln über den traditionellen katholischen Glauben. Hat die Kirche wirklich...? Antworten auf Irrtümer, Gebete, uvm.",
        _ => "",
    }.into()
}

fn generate_dropcap_css(a: &ParsedArticleAnalyzed) -> String {
    
    if a.is_prayer() {
        return String::new();
    }

    let try1 = a.article_abstract.get(0).map(par2text).unwrap_or_default();
    let sec1 = a.sections.get(0).and_then(|s| s.pars.get(0)).map(par2text).unwrap_or_default();
    let mut c = None;
    if !try1.trim().is_empty() {
        c = try1.trim().chars().next()
    } else if !sec1.trim().is_empty() {
        c = sec1.trim().chars().next();
    }

    let c = match c {
        Some(s) => if s.is_ascii_alphabetic() { 
            s.to_ascii_uppercase() 
        } else { return String::new(); },
        _ => return String::new(),
    };


    let dropcap_map = &[
        ('A', "U+0041"),
        ('B', "U+0042"),
        ('C', "U+0043"),
        ('D', "U+0044"),
        ('E', "U+0045"),
        ('F', "U+0046"),
        ('G', "U+0047"),
        ('H', "U+0048"),
        ('I', "U+0049"),
        ('J', "U+004A"),
        ('K', "U+004B"),
        ('L', "U+004C"),
        ('M', "U+004D"),
        ('N', "U+004E"),
        ('O', "U+004F"),
        ('P', "U+0050"),
        ('Q', "U+0051"),
        ('R', "U+0052"),
        ('S', "U+0053"),
        ('T', "U+0054"),
        ('U', "U+0055"),
        ('V', "U+0056"),
        ('W', "U+0057"),
        ('X', "U+0058"),
        ('Y', "U+0059"),
        ('Z', "U+005A"),
    ];

    let unicode_range = match dropcap_map.iter().find(|s| c == s.0).map(|q| q.1) {
        Some(s) => s,
        None => return String::new(),
    };

    let text = vec![
        "@font-face {".to_string(),
        "    font-family: 'Kanzlei Initialen';".to_string(),
        format!("    src: url('/static/font/dropcap/kanzlei/Kanzlei-Initialen-{c}.ttf') format('truetype');"),
        "    font-display: swap;".to_string(),
        format!("    unicode-range: {unicode_range};"),
        "}".to_string(),
    ];

    text.join("\r\n")
}

fn strip_comments(s: &str) -> String {
    let mut inside = false;
    let chars = s.chars().collect::<Vec<_>>();
    let mut c = Vec::new();
    let mut i = chars.iter().peekable();
    while let Some(a) = i.next() {
        if *a == '/' {
            if i.peek().copied().copied() == Some('*') {
                let _ = i.next();
                inside = true;
                continue;
            }
        } else if *a == '*' {
            if i.peek().copied().copied() == Some('/') {
                let _ = i.next();
                inside = false;
                continue;
            }
        }

        if !inside {
            c.push(*a);
        }
    }
    c.into_iter().collect()
}

fn minify_css(s: &str) -> String {
    use minifier::css;
    let s = strip_comments(s);
    let s = s.replace("xmlns=\"http://www.w3.org/2000/svg\"", "");
    let s = match css::minify(&s) {
        Ok(o) => o.to_string(),
        Err(e) => {
            println!("error cssmin: {e:?}");
            let _ = std::fs::write("./output.css", &s);
            s.to_string()
        },  
    };
    s
}

fn head(
    a: &ParsedArticleAnalyzed, 
    lang: &str,
    title_id: &str
) -> String {

    let darklight = include_str!("../../templates/darklight.html");
    let head_css = include_str!("../../static/css/head.css");
    let style_css = include_str!("../../static/css/style.css");
    let critical_css = minify_css(&(head_css.to_string() + "\r\n" + &style_css));
    let critical_css_2 = "<style id='critical-css'>\r\n".to_string() + &critical_css + "    </style>";

    let mut head = include_str!("../../templates/head.html").to_string();
    head = head.replace("<!-- DARKLIGHT_STYLES -->", &darklight);
    head = head.replace("<!-- CRITICAL_CSS -->", &critical_css_2);
    let title = get_title(lang, a);
    let description = get_description(lang, a).replace("\"", "'");

    head = head.replace("$$TITLE$$", &title);
    head = head.replace("$$DESCRIPTION$$", &description);
    head = head.replace("$$TITLE_ID$$", title_id);

    let drc = format!("<style>{}</style>", generate_dropcap_css(a));
    head = head.replace("<!-- DROPCAP_CSS -->", &drc);
    head = head.replace("$$KEYWORDS$$", &a.tags.join(", "));
    head = head.replace("$$DATE$$", &a.date);
    head = head.replace("$$AUTHOR$$", &a.authors.join(", "));

    head = head.replace("$$IMG$$", &a.img.as_ref().map(|s| s.href.clone()).unwrap_or_default());
    head = head.replace("$$IMG_ALT$$", &a.img.as_ref().map(|s| s.title.clone()).unwrap_or_default());
    head = head.replace("$$LANG$$", lang);
    head = head.replace("$$ROOT_HREF$$", &get_root_href());
    let page_href = get_root_href().to_string() + "/" + lang + "/" + title_id;
    head = head.replace("$$PAGE_HREF$$", &page_href);

    let skip = match lang {
        "de" => "Zum Hauptinhalt springen",
        "en" => "Skip to main content",
        _ => "",
    };
    head = head.replace("$$SKIP_TO_MAIN_CONTENT$$", skip);

    let contact = match lang {
        "en" => "en/about",
        "de" => "de/impressum",
        _ => "",
    };
    head = head.replace("$$CONTACT_URL$$", contact);
    head = head.replace("$$SLUG$$", title_id);
    head
}

fn header_navigation(
    lang: &str, 
    display_logo: bool
) -> String {
    use collection_macros::btreemap;
    
    let descs = btreemap!{
        "de" => btreemap!{
            "homepage_desc" => "Startseite: Kategorisierte Liste aller deutschen Artikel",
            "all_articles_desc" => "Durchsuche alle deutschen Artikel",
            "all_articles_title" => "Themen",
            "shop_desc" => "Unterstütze unser Apostolat mit deinem Einkauf in unserem Shop!",
            "shop_title" => "Shop",
            "shop_link" =>  "de/shop",
            "about_desc" => "Über diese Seite, Kontakt und Rechtliches",
            "about_title" => "Impressum",
            "tools_desc" => "Software und Hilfsmittel zum Latein-Lernen, Gebetssammlungen, Online Mess- und Bibelbücher, Online-Bücherei u.v.m",
            "tools_title" => "Ressourcen",
            "newest_desc" => "Artikel sortiert nach Datum",
            "newest_title" => "Neues",
            "about_link" => "de/impressum",
            "homepage_link" => "de",
            "all_articles_link" => "de/themen",
            "newest_link" => "de/neues",
            "tools_link" => "de/ressourcen",
        },
        "en" => btreemap!{
            "homepage_desc" => "Homepage",
            "all_articles_desc" => "Search all English articles by category / tag",
            "all_articles_title" => "Topics",
            "shop_desc" => "Support our apostolate with your purchase in our store!",
            "shop_title" => "Shop",
            "shop_link" => "en/shop",
            "about_desc" => "Imprint, contact and legal information",
            "about_title" => "About",
            "tools_desc" => "Software and aids for learning Latin, prayer collections, online Mass and Bible books, online library and much more",
            "tools_title" => "Tools",
            "newest_desc" => "Articles sorted by date",
            "newest_title" => "Newest",
            "about_link" => "en/about",
            "homepage_link" => "en",
            "all_articles_link" => "en/topics",
            "newest_link" => "en/newest",
            "tools_link" => "en/tools",
        },
    };

    let logo = if display_logo {
        let homepage_logo = "/static/img/logo/logo-smooth.svg#logo";
        let homepage_link = get_root_href().to_string() + "/" + lang;
        let hpd = descs[lang]["homepage_desc"];
        let logo1 = format!("<a class='logo has-content' rel='home me contents' href='{homepage_link}' data-attribute-title='{hpd}'>");
        let logo2 = format!("<svg class='logo-image' viewBox='0 0 64 75'><use href='{homepage_logo}'></use></svg>");
        vec![logo1, logo2, "</a>".to_string()].join("")
    } else {
        String::new()
    };
    
    let mut header_nav = include_str!("../../templates/header-navigation.html").to_string();
    header_nav = header_nav.replace("$$HOMEPAGE_LOGO$$", &logo);
    header_nav = header_nav.replace("$$TOOLS_DESC$$", descs[lang]["tools_desc"]);
    header_nav = header_nav.replace("$$TOOLS_TITLE$$", descs[lang]["tools_title"]);
    header_nav = header_nav.replace("$$TOOLS_LINK$$", descs[lang]["tools_link"]);
    header_nav = header_nav.replace("$$ABOUT_DESC$$", descs[lang]["about_desc"]);
    header_nav = header_nav.replace("$$ABOUT_TITLE$$", descs[lang]["about_title"]);
    header_nav = header_nav.replace("$$ABOUT_LINK$$", descs[lang]["about_link"]);
    header_nav = header_nav.replace("$$ALL_ARTICLES_TITLE$$", descs[lang]["all_articles_title"]);
    header_nav = header_nav.replace("$$ALL_ARTICLES_DESC$$", descs[lang]["all_articles_desc"]);
    header_nav = header_nav.replace("$$ALL_ARTICLES_LINK$$", descs[lang]["all_articles_link"]);
    header_nav = header_nav.replace("$$NEWEST_DESC$$", descs[lang]["newest_desc"]);
    header_nav = header_nav.replace("$$NEWEST_TITLE$$", descs[lang]["newest_title"]);
    header_nav = header_nav.replace("$$NEWEST_LINK$$", descs[lang]["newest_link"]);
    header_nav = header_nav.replace("$$SHOP_DESC$$", descs[lang]["shop_desc"]);
    header_nav = header_nav.replace("$$SHOP_TITLE$$", descs[lang]["shop_title"]);
    header_nav = header_nav.replace("$$SHOP_LINK$$", descs[lang]["shop_link"]);
    header_nav
}

fn link_tags(
    lang: &str, 
    tags: &[String]
) -> String {

    let root_href = get_root_href();

    let tags_str = tags.iter().map(|t| {
        let t_descr = match lang {
            "en" => "Link to ".to_string() + t + " tag",
            "de" => "Link zum Thema ".to_string() + t,
            _ => "".to_string(),
        };
        let t_url = match lang {
            "en" => "en/topics",
            "de" => "de/themen",
            _ => "",
        };
        let t1 = format!("<a href='{root_href}/{t_url}#{t}'");
        let t2 = "class='link-tag link-page link-annotated icon-not has-annotation spawns-popup' rel='tag' ";
        let t3 = format!(" data-attribute-title='{t_descr}'>{t}</a>");
        t1 + t2 + &t3
    }).collect::<Vec<_>>().join(", ");
    format!("<div class='link-tags'><p>{tags_str}</p></div>")
}

fn gen_section_id(s: &str) -> String {
    s.chars().filter_map(|c| if c.is_ascii_alphanumeric() {
        Some(c.to_ascii_lowercase())
    } else if c.is_whitespace() {
        Some('-')
    } else {
        None
    }).collect()
}

fn table_of_contents(
    lang: &str, 
    a: &ParsedArticleAnalyzed,
) -> String {
    if a.is_prayer() {
        return String::new();
    }

    if a.sections.is_empty() {
        return String::new();
    }
    
    let mut target = "<div id='TOC' class='TOC'>".to_string();
    target += "<ul class='list-level-1'>";
    let mut cur_level = a.sections[0].indent;
    let orig_cur_level = cur_level;

    for section in a.sections.iter() {
        let header = &section.title;
        let level = section.indent;
        let section_id = gen_section_id(&section.title);

        if level > cur_level {
            target += &format!("<ul class='list-level-{}'>", level - 1);
        }

        while level < cur_level {
            target += "</ul>";
            cur_level -= 1;
        }

        cur_level = level;
        target += "<li>";
        target += &format!(
            "<a href='#{section_id}' id='toc-{section_id}' class='decorate-not has-content spawns-popup'>{header}</a>"
        );
        target += "</li>";
    }

    while orig_cur_level < cur_level {
        target += "</ul>";
        cur_level -= 1;
    }

    let footnotes_id = "footnotes";
    let similar_id = "similar";
    let bibliography_id = "bibliography";
    let backlinks_id = "backlinks";

    let meta = btreemap! {
        "en" => btreemap! {
            "collapse_button_title" => "Collapse table of contents",
            "footnotes_title" => "Footnotes",
            "similar_title" => "Similar articles",
            "bibliography_title" => "Bibliography",
            "backlinks_title" => "Backlinks",
        },
        "de" => btreemap! {
            "collapse_button_title" => "Inhaltsverzeichnis zusammenklappen",
            "footnotes_title" => "Fußnoten",
            "similar_title" => "Ähnliche Artikel",
            "bibliography_title" => "Bibliographie",
            "backlinks_title" => "Verweise",
        }
    };

    let collapse_button_title = meta[lang]["collapse_button_title"];
    let footnotes_title = meta[lang]["footnotes_title"];
    let similar_title = meta[lang]["similar_title"];
    let bibliography_title = meta[lang]["bibliography_title"];
    let backlinks_title = meta[lang]["backlinks_title"];

    let s = "class='link-self decorate-not has-content spawns-popup'";

    target += &format!("<li><a {s} id='toc-backlinks' href='#{backlinks_id}'>{backlinks_title}</a></li>");
    target += &format!("<li><a {s} id='toc-footnotes' href='#{footnotes_id}'>{footnotes_title}</a></li>");
    target += &format!("<li><a {s} id='toc-similar' href='#{similar_id}'>{similar_title}</a></li>");
    target += &format!("<li><a {s} id='toc-bibliography' href='#{bibliography_id}'>{bibliography_title}</a></li>");
    target += &format!("</ul>");
    target += &format!("<button class='toc-collapse-toggle-button' title='{collapse_button_title}' tabindex='-1'><span></span></button>");
    target += &format!("</div>");

    target
}

fn page_desciption(
    lang: &str, 
    a: &ParsedArticleAnalyzed,
) -> String {
    if a.is_prayer() {
        return String::new();
    }
    let descr = get_description(lang, a);
    format!("<div class='page-description' style='max-width: 600px;margin: 0 auto;'><p>{descr}</p></div>")
}

type AuthorsMap = BTreeMap<String, Author>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Author {
    displayname: String,
    #[serde(default)]
    contact: Option<String>,
    #[serde(default)]
    donate: BTreeMap<String, String>
}

fn read_authors_map(s: &str) -> AuthorsMap {
    serde_json::from_str(&s).unwrap_or_default()
}

fn page_metadata(
    lang: &str, 
    a: &ParsedArticleAnalyzed,
    authors: &AuthorsMap,
) -> Result<String, String> {

    if a.is_prayer() {
        return Ok(String::new());
    }

    let mut page_meta = include_str!("../../templates/page-metadata.html").to_string();
    let date = a.date.clone();
    let date_desc = date.clone();
    let date_title = date.clone();

    let authors_link = a.authors.iter().map(|s| {
        let id = s.replace(":", "-");
        let name = authors.get(s).map(|q| &q.displayname)
        .ok_or_else(|| format!("author {s} not found for article {}", a.title))?;
        
        let u = "/static/img/icon/icons.svg#info-circle-regular";
        let style = format!("data-link-icon='info-circle-regular' data-link-icon-type='svg' style=\"--link-icon-url: url('{u}');\"");
        let classes = "class='backlinks link-self has-icon has-content spawns-popup has-indicator-hook'";
        
        let mut link = format!("<a href='/{lang}/author/{id}' data-attribute-title='{name}' {style} {classes}>");
        link += &format!("{name}<span class='indicator-hook'></span><span class='link-icon-hook'>⁠</span></a>");

        Ok(link)
    }).collect::<Result<Vec<_>, String>>()?.join(", ");

    let meta = btreemap! {
        "de" => btreemap! {
            "backlinks_desc" => "Liste der anderen Seiten, die auf diese Seite verweisen",
            "backlinks_title" => "verweise",
            "similar_desc" => "Ähnliche Artikel",
            "similar_title" => "ähnlich",
            "bibliography_desc" => "Bibliographie der auf dieser Seite zitierten Links",
            "bibliography_title" => "bibliografie",
        },
        "en" => btreemap! {
            "backlinks_desc" => "List of other pages which link to this page",
            "backlinks_title" => "backlinks",
            "similar_desc" => "Similar articles for this link",
            "similar_title" => "similar",
            "bibliography_desc" => "Bibliography of links cited in this page",
            "bibliography_title" => "bibliography",
        },
    };

    let backlinks_desc = meta[lang]["backlinks_desc"];
    let backlinks_title = meta[lang]["backlinks_title"];
    let similar_desc = meta[lang]["similar_desc"];
    let similar_title = meta[lang]["similar_title"];
    let bibliography_desc = meta[lang]["bibliography_desc"];
    let bibliography_title = meta[lang]["bibliography_title"];

    page_meta = page_meta.replace("$$DATE_DESC$$", &date_desc);
    page_meta = page_meta.replace("$$DATE_TITLE$$", &date_title);
    page_meta = page_meta.replace("$$BACKLINKS_DESC$$", backlinks_desc);
    page_meta = page_meta.replace("$$BACKLINKS_TITLE$$", backlinks_title);
    page_meta = page_meta.replace("$$SIMILAR_DESC$$", similar_desc);
    page_meta = page_meta.replace("$$SIMILAR_TITLE$$", similar_title);
    page_meta = page_meta.replace("$$BIBLIOGRAPHY_DESC$$", bibliography_desc);
    page_meta = page_meta.replace("$$BIBLIOGRAPHY_TITLE$$", bibliography_title);
    page_meta = page_meta.replace("<!-- AUTHORS -->", &authors_link);
    
    Ok(page_meta)
}

fn render_paragraph(
    lang: &str,
    par: &Paragraph, 
    dropcap: bool, 
    is_abstract: bool, 
    article_id: &str
) -> String {
    let mut target = String::new();
    match par {
        Paragraph::Sentence { s } => {
            for (i, item) in s.iter().enumerate() {
                match item {
                    SentenceItem::Text { text } => {
                        if dropcap && i == 0 {
                            let drc = text.chars().next().map(|s| s.to_string()).unwrap_or_default();
                            target += &format!("<span class='dropcap'>{drc}</span>");
                            let rest = text.chars().skip(1).collect::<String>();
                            target += &rest;
                        } else {
                            target += &text;
                        }
                    }
                    SentenceItem::Link { l } => {
                        target += &format!("<a class='link-annotated link-page has-icon has-annotation spawns-popup' href='{}'>{}</a>", l.href, l.text);
                    }
                    SentenceItem::Footnote { id } => {
                        // TODO!
                    }
                }
            }
        },
        Paragraph::Quote { q } => {
            let lv = if is_abstract { "2" } else { "1" };
            target += &format!("<blockquote class='blockquote-level-{lv}' style='margin-top:10px;margin-bottom: 10px;'>");
            if !q.title.is_empty() {
                target += "<strong>";
                target += &q.title;
                target += "</strong>";
            }

            for p in q.quote.iter() {
                target += "<p class='first-block first-graf'>";
                target += p;
                target += "</p>";
            }
            
            if !(q.author.is_empty() && q.source.is_empty()) {
                target += "<em style='padding-left:10px;'>";
                if !q.author.is_empty() {
                    target += &format!("<a class='link-annotated link-page has-icon has-annotation spawns-popup' href='{}'>{}</a>", q.author_link, q.author);
                }
    
                if !q.source.is_empty() {
                    if !q.author.is_empty() {
                        target += "&nbsp;—&nbsp;";
                    }
                    target += &format!("<a class='link-annotated link-page has-icon has-annotation spawns-popup' href='{}'>{}</a>", q.source_link, q.source);
                }
                target += "</em>"
            }

            target += "</blockquote>"
        },
        Paragraph::Image { i } => {

            
            let href = if i.href.contains("://") {
                i.href.clone()
            } else {
                get_root_href().to_string() + "/articles/" + lang + "/" + article_id + "/" + &i.href
            };

            // TODO: inline images
            target += &render_image(&Image { 
                href: href, 
                alt: i.alt.clone(), 
                title: i.title.clone(),
                inline: i.inline,
            });
        },
    }

    target
}

fn render_image(i: &Image) -> String {

    // TODO: width="1400" height="1400" data-aspect-ratio="1 / 1" style="aspect-ratio: 1 / 1; width: 678px;"
    let template = match !i.inline {
        true => include_str!("../../templates/figure.float.html").replace("$$DIRECTION$$", "right"),
        false => include_str!("../../templates/figure.html").to_string(),
    };

    template
    .replace("$$IMG_ALT$$", &i.alt)
    .replace("$$IMG_HREF$$", &i.href)
    .replace("$$IMG_CAPTION$$", &i.title)
}

fn body_abstract(
    lang: &str,
    article_id: &str,
    summary: &[Paragraph],
) -> String {
    
    if summary.is_empty() {
        return String::new();
    }

    let mut target = "<blockquote class='blockquote-level-1 block'>".to_string();
    target += "<p class='first-block first-graf intro-graf dropcap-kanzlei' style='--bsm: 0;display:block;min-height:7em;'>";
    target += &render_paragraph(lang, &summary[0], true, true, article_id); 
    target += "</p>";

    for par in summary.iter().skip(1) {
        target += "<p style='--bsm: 0;'>";
        target += &render_paragraph(lang, par, false, true, article_id);
        target += "</p>";
    }

    target += "</blockquote>";
    target
}

fn render_section(
    lang: &str,
    a: &ArticleSection,
    slug: &str,
) -> String {
    
    let mut section = include_str!("../../templates/section.html").to_string();

    let first_par = a.pars.get(0).map(|p| render_paragraph(lang, p, false, false, slug)).unwrap_or_default();
    let other_pars = a.pars.iter().skip(1).map(|p| render_paragraph(lang, p, false, false, slug)).collect::<Vec<_>>().join("\r\n");

    let header = &a.title;
    let level = a.indent;
    let section_id = gen_section_id(&header);
    let section_descr = match lang {
        "de" => format!("Link zum Abschnitt '{header}'"),
        "en" => format!("Link to section '{header}'"),
        _ => String::new(),
    };

    section = section.replace("$$LEVEL$$", &level.saturating_sub(1).to_string());
    section = section.replace("$$SECTION_ID$$", &section_id);
    section = section.replace("$$SECTION_DESCR$$", &section_descr);
    section = section.replace("$$SECTION_TITLE$$", &header);
    section = section.replace("<!-- FIRST_PARAGRAPH -->", &first_par);
    
    section += &other_pars;

    section
}

fn body_content(
    lang: &str,
    slug: &str,
    sections: &[ArticleSection],
) -> String {
    sections.iter()
    .map(|q| render_section(lang, q, slug))
    .collect::<Vec<_>>()
    .join("\r\n")
}

fn body_noscript() -> String {
    include_str!("../../templates/body-noscript.html").to_string()
}

fn article2html(
    lang: &str, 
    slug: &str, 
    a: &ParsedArticleAnalyzed, 
    prod: bool,
    articles_by_tag: &mut ArticlesByTag,
    articles_by_date: &mut ArticlesByDate,
    authors: &AuthorsMap,
) -> Result<String, String> {
    
    static HTML: &str = include_str!("../../templates/lorem.html");

    match (lang, slug) {
        ("de", "rosenkranz") |
        ("en", "rosary") => return Err(String::new()), // TODO
        _ => { },
    }

    for t in a.tags.iter() {
        articles_by_tag.entry(lang.to_string())
        .or_insert_with(|| BTreeMap::new())
        .entry(t.to_string())
        .or_insert_with(|| Vec::new())
        .push(SectionLink { slug: slug.to_string(), title: a.title.to_string() });
    }

    if !a.is_prayer() {
        match a.get_date() {
            Some((y, m, d)) => {
                articles_by_date
                .entry(lang.to_string())
                .or_insert_with(|| BTreeMap::new())
                .entry(y.to_string())
                .or_insert_with(|| BTreeMap::new())
                .entry(m.to_string())
                .or_insert_with(|| BTreeMap::new())
                .entry(d.to_string())
                .or_insert_with(|| Vec::new())
                .push(SectionLink { slug: slug.to_string(), title: a.title.to_string() });
            },
            None => {
                println!("article {lang}/{slug} has no date");
            }
        };

    }

    let title_id = lang.to_string() + "-" + slug;
    
    let html = HTML.replace("<!-- HEAD_TEMPLATE_HTML -->", &head(a, lang, title_id.as_str()));
    let html = html.replace("<!-- HEADER_NAVIGATION -->", &header_navigation(lang, true));
    let html = html.replace("<!-- LINK_TAGS -->", &link_tags(lang, &a.tags));
    let html = html.replace("<!-- TOC -->", &table_of_contents(lang, &a));
    let html = html.replace("<!-- PAGE_DESCRIPTION -->", &page_desciption(lang, &a));
    let html = html.replace("<!-- PAGE_METADATA -->", &page_metadata(lang, &a, authors)?);
    let html = html.replace("<!-- BODY_ABSTRACT -->", &body_abstract(lang, slug, &a.article_abstract));
    let html = html.replace("<!-- BODY_CONTENT -->", &body_content(lang, &slug, &a.sections));
    let html = html.replace("<!-- BODY_NOSCRIPT -->", &body_noscript());

    let skip = match lang {
        "de" => "Zum Hauptinhalt springen",
        "en" => "Skip to main content",
        _ => return Err(format!("unknown language {lang}")),
    };
    let html = html.replace("$$SKIP_TO_MAIN_CONTENT$$", skip);
    let contact = match lang {
        "de" => "de/impressum",
        "en" => "en/about",
        _ => return Err(format!("unknown language {lang}")),
    };
    
    let root_href = match prod {
        true => "https://dubia.cc",
        false => "http://127.0.0.1:8080",
    };

    let html = html.replace("$$CONTACT_URL$$", contact);
    let html = html.replace("$$TITLE$$", &a.title);
    let html = html.replace("$$TITLE_ID$$", &title_id);
    let html = html.replace("$$LANG$$", &lang);
    let html = html.replace("$$SLUG$$", slug);
    let html = html.replace("$$ROOT_HREF$$", root_href);
    let html = html.replace("$$PAGE_HREF$$", &(root_href.to_string() + "/" + slug));

    Ok(html)
}

fn render_page_author_pages(
    articles: &AnalyzedArticles,
    authors: &AuthorsMap
) -> Result<BTreeMap<String, Vec<(String, String)>>, String> {
    
    let mut finalmap = BTreeMap::new();
    for lang in articles.map.keys() {
        
        let (contact_str, donate_str) = match lang.as_str() {
            "de" => ("Kontakt", "Spenden"),
            "en" => ("Contact", "Donate"),
            _ => return Err("unknown language".to_string()),
        };
    
        for (id, v) in authors.iter() {
            let name = &v.displayname;
            let contact_url = v.contact.as_deref();
            let mut dn = String::new();
            for (platform, link) in v.donate.iter() {
                
                let s = match platform.as_str() {
                    "paypal" => format!("<p><a href='{link}'>PayPal</a></p>"),
                    "github" => format!("<p><a href='{link}'>Ko-Fi</a></p>"),
                    "ko-fi" => format!("<p><a href='{link}'>GitHub Sponsors</a></p>"),
                    _ => return Err(format!("unknown platform {platform} for user {id} in authors.json")),
                };
            
                dn.push_str(&s);
            }
            
            let mut t = format!("<!doctype html><html><head><title>{name}</title></head><body>");
            t += &format!("<h1>{name}</h1>");
            if let Some(contact_url) = contact_url {
                t += &format!("<h2>{contact_str}</h2>");
                t += &format!("<a href='{contact_url}'>{contact_url}</a>");
            }

            if !dn.is_empty() {
                t += &format!("<h2>{donate_str}</h2>");
                t += &dn;
            }
            t += &format!("</body></html>");

            finalmap.entry(lang.clone())
            .or_insert_with(|| Vec::new())
            .push((id.to_lowercase().replace(":", "-"), t));
        }
    }

    Ok(finalmap)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct SearchIndex {
    git: String,
    articles: BTreeMap<Slug, SearchIndexArticle>
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct SearchIndexArticle {
    title: String,
    sha256: String,
}

fn generate_search_index(articles: &AnalyzedArticles) -> BTreeMap<Lang, SearchIndex>{

    articles.map.iter().map(|(lang, a)| {

        let s = a.values().map(|r| r.sha256.clone()).collect::<Vec<_>>().join(" ");
        let version = sha256(&s);
        let articles = a.iter()
        .map(|(slug, readme)| {
            let sia = SearchIndexArticle {
                title: readme.title.clone(),
                sha256: readme.sha256.clone(),
            };
            (slug.clone(), sia)
        }).collect();

        (lang.clone(), SearchIndex {
            git: version,
            articles,
        })
    }).collect()
}

type SearchHtmlResult = BTreeMap<Lang, (String, String, String)>;

// Lang => (SearchBarHtml, SearchJS)
fn search_html(articles: &AnalyzedArticles) -> SearchHtmlResult {
    articles.map.iter().map(|(lang, a)| {

        let s = a.values().map(|r| r.sha256.clone()).collect::<Vec<_>>().join(" ");
        let version = sha256(&s);

        let searchbar_placeholder = match lang.as_str() {
            "de" => "Stichwort, Thema, Frage, ...",
            "en" => "Keyword, topic, question, ...",
            _ => "",
        };

        let searchbar = match lang.as_str() {
            "de" => "Suchen",
            "en" => "Search",
            _ => "",
        };

        let no_results = match lang.as_str() {
            "de" => "Keine Ergebnisse gefunden.",
            "en" => "No results found.",
            _ => "",
        };

        let mut searchbar_html = include_str!("../../templates/searchbar.html").to_string();
        searchbar_html = searchbar_html.replace("$$VERSION$$", &version);
        searchbar_html = searchbar_html.replace("$$SEARCHBAR_PLACEHOLDER$$", &searchbar_placeholder);
        searchbar_html = searchbar_html.replace("$$SEARCH$$", &searchbar);
        
        let mut search_html = include_str!("../../templates/search.html").to_string();
        search_html = search_html.replace("<!-- SEARCH -->", &searchbar_html);
        search_html = search_html.replace("<!-- HEADER_NAVIGATION -->", &header_navigation(lang, true));
        search_html = search_html.replace("$$LANG$$", lang);
        search_html = search_html.replace("$$ROOT_HREF$$", &get_root_href());
        let title = match lang.as_str() {
            "de" => "Suche - dubia.cc",
            "en" => "Search - dubia.cc",
            _ => "",
        };
        let description = match lang.as_str() {
            "de" => "Durchsuche dubia.cc",
            "en" => "Search dubia.cc",
            _ => "",
        };
        let parsed = ParsedArticleAnalyzed {
            title: title.to_string(),
            summary: vec![Paragraph::Sentence { s: vec![SentenceItem::Text { text: description.to_string() }] }],
            .. Default::default()
        };
        search_html = search_html.replace("<!-- HEAD_TEMPLATE_HTML -->", &head(&parsed, lang, &format!("{lang}-search")));
        search_html = search_html.replace("$$TITLE$$", title);

        let mut search_js = include_str!("../../static/js/search.js").to_string();
        search_js = search_js.replace("$$LANG$$", lang);
        search_js = search_js.replace("$$VERSION$$", &version);
        search_js = search_js.replace("$$NO_RESULTS$$", no_results);

        (lang.clone(), (searchbar_html, search_html, search_js))
    }).collect()
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct IBelieveIn {
    title: String,
    option: String,
    tag: String,
    featured: Vec<Slug>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct IwantToLearn {
    title: String,
    featured: Vec<Slug>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct TagSection1 {
    id: String,
    title: String,
    links: Vec<SectionLink>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct TagSection2 {
    id: String,
    title: String,
    img: String,
    link: SectionLink,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct TagSection3 {
    id: String,
    title: String,
    texts: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct Tags {
    ibelievein: Vec<IBelieveIn>,
    iwanttolearn: BTreeMap<Slug, IwantToLearn>,
    tags: BTreeMap<String, String>,
    ressources: Vec<TagSection1>,
    shop: Vec<TagSection2>,
    about: Vec<TagSection3>,
}

struct SpecialPage {
    id: &'static str,
    filepath: &'static str,
    title: &'static str,
    description: &'static str,
    content: String,
}

fn get_special_pages(
    lang: &str,
    tags: &BTreeMap<Lang, Tags>,
    by_tag: &ArticlesByTag,
    by_date: &ArticlesByDate,
) -> Result<Vec<SpecialPage>, String> {
    let tags = tags.get(lang).ok_or_else(|| format!("unknown language {lang} not found in tags.json"))?;
    let default = BTreeMap::new();
    let default2 = BTreeMap::new();
    match lang {
        "de" => Ok(vec![
            SpecialPage {
                title: "Themen",
                filepath: "themen.html",
                id: "de-themen",
                description: "Themenübersicht",
                content: render_index_sections(
                    lang, 
                    by_tag.get(lang).unwrap_or(&default).iter().filter_map(|(k, v)| {
                        let id = k.clone();
                        let title = tags.tags.get(&id)?;
                        Some(((id.to_string(), title.to_string()), v.clone()))
                    }).collect()
                ),
            },
            SpecialPage {
                title: "Neueste Links",
                filepath: "neues.html",
                id: "de-neues",
                description: "Neueste Links",
                content: render_index_sections(lang, by_date.get(lang).unwrap_or(&default2).iter().rev().map(|(year, months)| {
                    ((format!("jahr-{year}"), year.clone()), months.iter().flat_map(|(m, days)| {
                        days.iter().flat_map(move |(d, a)| a.iter().map(move |a| {
                            SectionLink {
                                slug: a.slug.to_string(),
                                title: format!("{m}-{d}: {}", a.title),
                            }
                        }))
                    }).collect())
                }).collect()),
            },
            SpecialPage {
                title: "Ressourcen",
                filepath: "ressourcen.html",
                id: "de-ressourcen",
                description: "Ressourcen",
                content: render_resources_sections(lang, &tags.ressources),
            },
            SpecialPage {
                title: "Shop",
                filepath: "shop.html",
                id: "de-shop",
                description: "Shop",
                content: render_shop_sections(&tags.shop),
            },
            SpecialPage {
                title: "Impressum",
                filepath: "impressum.html",
                id: "de-impressum",
                description: "Impressum",
                content: render_about_sections(&tags.about),
            },
        ]),
        "en" => Ok(vec![
            SpecialPage {
                title: "Topics",
                filepath: "topics.html",
                id: "en-topics",
                description: "List of topics",
                content: render_index_sections(
                    lang, 
                    by_tag.get(lang).unwrap_or(&default).iter().filter_map(|(k, v)| {
                        let id = k.clone();
                        let title = tags.tags.get(&id)?;
                        Some(((id.to_string(), title.to_string()), v.clone()))
                    }).collect()
                ),
            },
            SpecialPage {
                title: "Newest Links",
                filepath: "newest.html",
                id: "en-newest",
                description: "Newest Links",
                content: render_index_sections(lang, by_date.get(lang).unwrap_or(&default2).iter().map(|(year, months)| {
                    ((format!("year-{year}"), year.clone()), months.iter().flat_map(|(m, days)| {
                        days.iter().flat_map(move |(d, a)| a.iter().map(move |a| {
                            SectionLink {
                                slug: a.slug.to_string(),
                                title: format!("{m}-{d}: {}", a.title),
                            }
                        }))
                    }).collect())
                }).collect()),
            },
            SpecialPage {
                title: "Tools",
                filepath: "tools.html",
                id: "en-tools",
                description: "Tools",
                content: render_resources_sections(lang, &tags.ressources),
            },
            SpecialPage {
                title: "Shop",
                filepath: "shop.html",
                id: "en-shop",
                description: "Shop",
                content: render_shop_sections(&tags.shop),
            },
            SpecialPage {
                title: "About",
                filepath: "about.html",
                id: "en-about",
                description: "About",
                content: render_about_sections(&tags.about),
            },
        ]),
        _ => Err(format!("get_special_pages: unknown language {lang}"))
    }
}

fn special2html(lang: &str, page: &SpecialPage) -> (String, String) {
    let mut special = include_str!("../../templates/special.html").to_string();
    let a = ParsedArticleAnalyzed {
        title: page.title.to_string(),
        summary: vec![Paragraph::Sentence { s: vec![SentenceItem::Text { text: page.description.to_string() } ] }],
        .. Default::default()
    };
    special = special.replace("<!-- HEAD_TEMPLATE_HTML -->", &head(&a, lang, &page.id));
    special = special.replace("<!-- BODY_ABSTRACT -->", &page.content);
    special = special.replace("<!-- HEADER_NAVIGATION -->", &header_navigation(lang, true));
    special = special.replace("$$TITLE$$", &page.title);
    special = special.replace("$$LANG$$", lang);
    special = special.replace("$$ROOT_HREF$$", &get_root_href());
    (page.filepath.to_string(), special)
}

fn tags_map(s: &str) -> Result<BTreeMap<String, Tags>, String> {
    serde_json::from_str(s).map_err(|e| format!("tags.json: {}", e))
}

fn render_section_items_texts(texts: &[String]) -> String {
    texts.iter().map(|s| {
        if s.trim().is_empty() {
            "<br/>".to_string()
        } else if !s.trim().starts_with("<") {
            format!("<p style='text-indent: 0px;'>{s}</p>")
        } else {
            s.clone()
        }
    }).collect::<Vec<_>>().join("\r\n")
}

fn render_section_items_img(link: &str, img: &str, title: &str) -> String {
    let s1 = "justify-content: flex-end;margin-top:10px;width: 100%;min-height: 440px;display: flex;";
    let s2 = "flex-direction:column;height: 100%;background-size: cover;";
    let style = format!("{s1}{s2}background-image: url({img});");

    let p1 = "font-variant-caps: small-caps;background: var(--background-color);border-radius:5px;";
    let p2 = "border: 2px solid var(--GW-H1-border-color); text-align: center; text-decoration: underline;";
    let p3 = "text-indent: 0px;margin: 10px;padding: 10px 20px;";
    let p_style = p1.to_string() + p2 + p3;

    format!("<a href='{link}' style='{style}'><p style='{p_style}'>{title}</p></a>")
}

fn render_section_items(lang: &str, links: &[SectionLink]) -> String {
    links.iter().enumerate().map(|(i, l)| {
        let first = i == 0;
        let slug = &l.slug;
        let section_title = &l.title;
        let bsm = if !first { "0" } else { "4" };
        let final_link = if slug.starts_with("http") { 
            slug.clone() 
        } else { 
            get_root_href().to_string() + "/" + lang + "/" + slug 
        };

        vec![
            format!("<li class='block link-modified-recently-list-item dark-mode-invert' style='--bsm:{bsm};'>"),
            format!("  <p class='in-list first-graf block' style='--bsm: 0;'><a href='{final_link}'"),
            format!("      id='{lang}-{slug}'"),
            format!("      class='link-annotated link-page link-modified-recently in-list has-annotation spawns-popup'"),
            format!("      data-attribute-title='{section_title}'>{section_title}</a></p>"),
            format!("</li>"),
        ].join("\r\n")
    }).collect::<Vec<_>>().join("\r\n")
}

fn render_index_section(lang: &str, id: &str, classes: &str, title: &str, links: &[SectionLink]) -> String {
    let mut section_html = include_str!("../../templates/index.section.html").to_string();
    section_html = section_html.replace("$$SECTION_ID$$", id);
    section_html = section_html.replace("$$SECTION_CLASSES$$", classes);
    section_html = section_html.replace("$$SECTION_NAME$$", title);
    section_html = section_html.replace("$$SECTION_NAME_TITLE$$", title);
    section_html = section_html.replace("<!-- SECTION_ITEMS -->", &render_section_items(lang, links));
    section_html
}

fn render_index_section_texts(id: &str, classes: &str, title: &str, txts: &[String]) -> String {
    let mut section_html = include_str!("../../templates/index.section.html").to_string();
    section_html = section_html.replace("$$SECTION_ID$$", id);
    section_html = section_html.replace("$$SECTION_CLASSES$$", classes);
    section_html = section_html.replace("$$SECTION_NAME$$", title);
    section_html = section_html.replace("$$SECTION_NAME_TITLE$$", title);
    section_html = section_html.replace("<!-- SECTION_ITEMS -->", &&render_section_items_texts(txts));
    section_html
}

fn render_index_section_img(id: &str, classes: &str, title: &str, link: &str, img: &str, t: &str) -> String {
    let mut section_html = include_str!("../../templates/index.section.html").to_string();
    section_html = section_html.replace("$$SECTION_ID$$", id);
    section_html = section_html.replace("$$SECTION_CLASSES$$", classes);
    section_html = section_html.replace("$$SECTION_NAME$$", title);
    section_html = section_html.replace("$$SECTION_NAME_TITLE$$", title);
    section_html = section_html.replace("<!-- SECTION_ITEMS -->", &&render_section_items_img(link, img, t));
    section_html
}

fn render_index_sections(lang: &str, s: Vec<((String, String), Vec<SectionLink>)>) -> String {
    s.iter().map(|((id, title), links)| {
        render_index_section(lang, id, "", title, links)
    }).collect::<Vec<_>>().join("\r\n")
}

fn render_resources_sections(lang: &str, s: &Vec<TagSection1>) -> String {
    s.iter().map(|s| {
        let section_id = &s.id;
        let section_title = &s.title;
        render_index_section(lang, section_id, "", section_title, &s.links)
    }).collect::<Vec<_>>().join("\r\n")
}

fn render_shop_sections(s: &Vec<TagSection2>) -> String {
    s.iter().map(|s| {
        render_index_section_img(&s.id, "", &s.title, &s.link.slug, &s.img, &s.link.title)
    }).collect::<Vec<_>>().join("\r\n")
}

fn render_about_sections(s: &Vec<TagSection3>) -> String {
    s.iter().map(|s| {
        render_index_section_texts(&s.id, "", &s.title, &s.texts)
    }).collect::<Vec<_>>().join("\r\n")
}

fn render_index_first_section(
    lang: &str,
    tags: &Tags,
    articles: &AnalyzedArticles,
) -> String {

    let mut first_section = include_str!("../../templates/index.first-section.html").to_string();
    let mut other_sections = "<style>.section-hidden { display: none; }</style>".to_string();
    let mut sections = String::new();
    let mut options = String::new();

    for (i, t) in tags.ibelievein.iter().enumerate() {

        let featured = t.featured.iter().filter_map(|id| {
            Some(SectionLink {
                title: articles.map.get(lang)?.get(id)?.title.clone(),
                slug: id.to_string(),
            })
        }).collect::<Vec<_>>();
        
        let items = render_section_items(lang, &featured);
        let first = i == 0;
        let display_hidden = match first {
            true => "",
            false => "display:none;",
        };
        let classes = "index-first-section list list-level-1";
        let tag = &t.tag;
        let option = &t.option;
        sections += &format!("<ul id='index-section-{tag}' class='{classes}' style='margin-top:20px;{display_hidden}'>{items}</ul>");
        options += &format!("<option value='{tag}'>{option}</option>");
        /*
        let other_sections_html = tags.ibelievein.iter()
            .filter(|q| q.tag != t.tag)
            .map(|q| {

                let hidden_class = match first {
                    true => "",
                    false => "section-hidden",
                };

                let classes = format!("{hidden_class} invert invert-of-{tag}");
                let other_featured = q.featured.iter().filter_map(|id| {
                    Some(SectionLink {
                        title: articles.map.get(lang)?.get(id)?.title.clone(),
                        slug: id.to_string(),
                    })
                }).collect::<Vec<_>>();
                
                render_index_section(lang, &q.tag,  &classes, &q.title, &other_featured)
            }).collect::<Vec<_>>().join("");
        other_sections += &other_sections_html;
        */
    }



    let text_ibelieve = match lang {
        "de" => "Ich glaube an",
        "en" => "I believe in",
        _ => "",
    };
    first_section = first_section.replace("$$I_BELIEVE_IN$$", &text_ibelieve);
    first_section = first_section.replace("<!-- SECTIONS -->", &sections);
    first_section = first_section.replace("<!-- OTHER_SECTIONS -->", &other_sections);
    first_section = first_section.replace("<!-- OPTIONS -->", &options);
    first_section
    /*
        for t in tags[lang]["ibelievein"]:
            featured = []
            for f in t["featured"]:
                slug = f
                title = articles[lang + "/" + slug].get("title", "")
                featured.append({ "slug": slug, "title": title })
            t["featured"] = featured

        text_ibelieve = "I believe in"
        if lang == "de":
            text_ibelieve = "Ich glaube an"

        ibelievein = tags[lang]["ibelievein"]
        first_section = read_file("./templates/index.first-section.html")
        other_sections = "<style>.section-hidden { display: none; }</style>"
        sections = ""
        options = ""
        first = True

        for t in ibelievein:
            li_items = render_section_items(lang, t["featured"])
            display_hidden = "display:none;"
            if first:
                display_hidden = ""
            classes = "index-first-section list list-level-1"
            sections += "<ul id='index-section-" + t["tag"] + "' class='" + classes + "' style='margin-top:20px;" + display_hidden + "'>" + li_items + "</ul>"
            options += "<option value='" + t["tag"] + "'>" + t["option"] + "</option>"
            for q in ibelievein:
                if t["tag"] == q["tag"]:
                    continue
                hidden_class = "section-hidden"
                if first:
                    hidden_class = ""
                other_sections += render_index_section(lang, q["tag"],  hidden_class + " invert invert-of-" + t["tag"], q["title"], q["featured"])
            first = False

    */

}

fn render_other_index_sections(
    lang: &str,
    tags: &Tags,
    articles: &AnalyzedArticles,
) -> Result<String, String> {

    let articles = articles.map.get(lang)
    .ok_or_else(|| format!("render_other_index_sections: unknown language {lang}"))?;

    Ok(tags.iwanttolearn.iter().map(|(id, v)| {

        let featured = v.featured.iter().filter_map(|f_id| {
            let featured_title = articles.get(f_id)?.title.clone();
            Some(SectionLink {
                slug: f_id.to_string(),
                title: featured_title,
            })
        }).collect::<Vec<_>>();

        render_index_section(lang, id, "", &v.title, &featured)
    }).collect::<Vec<_>>().join(""))
}

fn render_index_html(
    lang: &str, 
    articles: &AnalyzedArticles, 
    tags: &BTreeMap<Lang, Tags>,
    search_html: &SearchHtmlResult,
) -> Result<String, String> {
    
    let tags = tags.get(lang)
    .ok_or_else(|| format!("render_index_html: unknown language {lang}"))?;

    let (searchbar_html, _, _) = search_html.get(lang)
    .ok_or_else(|| format!("render_index_html (searchbar_html): unknown language {lang}"))?;

    let multilang = include_str!("../../templates/multilang.tags.html");
    let logo_svg = include_str!("../../static/img/logo/full.svg");
    
    let title = get_title(lang, &ParsedArticleAnalyzed::default());
    let description = get_description(lang, &ParsedArticleAnalyzed::default());
    let keywords = match lang {
        "en" => vec![
            "catholic", 
            "church", 
            "church fathers", 
            "faith", 
            "meaning of life", 
            "evolution", 
            "protestant", 
            "islam"
        ],
        "de" => vec![
            "katholisch", 
            "kirche", 
            "kirchenväter", 
            "glauben", 
            "sinn des lebens", 
            "evolution", 
            "evangelisch", 
            "islam"
        ],
        _ => vec![],
    }.into_iter().map(String::from).collect();

    let a = ParsedArticleAnalyzed {
        title: title.clone(),
        summary: vec![Paragraph::Sentence { s: vec![SentenceItem::Text { text: description.clone() }] }],
        tags: keywords,
        .. Default::default()
    };

    let select_faith = match lang {
        "de" => "Glauben auswählen",
        "en" => "Select Faith",
        _ => "",
    };

    let page_help_content = match lang {
        "de" => vec![
            "Einstellungen zu <em>Nachtmodus</em> (<span class='icon-moon-solid'></span>),",
            "<em>Lesemodus</em> (<span class='icon-book-open-solid'></span>), ",
            "<em>Popups</em> (<span class='icon-message-slash-solid'></span>) und ",
            "<em>Suche</em> (<span class='icon-magnifying-glass'></span>) finden Sie im Menü in der ",
            "<span class='desktop-not'>unteren rechten</span>",
            "<span class='mobile-not'>oberen rechten</span> ",
            "Ecke (<span class='icon-gear-solid'></span>)",
        ],
        "en" => vec![
            "To use <em>dark-mode</em> (<span class='icon-moon-solid'></span>) or",
            "<em>reader-mode</em> (<span class='icon-book-open-solid'></span>), or ",
            "<em>disable popups</em> (<span class='icon-message-slash-solid'></span>), or ",
            "<em>search</em> the site (<span class='icon-magnifying-glass'></span>), use the floating toggle bar in the ",
            "<span class='desktop-not'>lower-right</span>",
            "<span class='mobile-not'>upper-right</span> ",
            "corner (<span class='icon-gear-solid'></span>)</p>",
        ],
        _ => vec![],
    }.join("");
    
    let page_help = include_str!("../../templates/navigation-help.html")
    .replace("$$PAGE_HELP$$", &page_help_content);

    let page_descr = match lang {
        "de" => vec![
            "<strong>dubia.cc</strong> ist die eine deutsche<br/>",
            "Sammlung an Artikeln über den<br/>",
            "traditionellen katholischen Glauben",
        ],
        "en" => vec![
            "<strong>dubia.cc</strong> is a collection",
            "<br/>of articles explaining the<br/>",
            "traditional Catholic faith",
        ],
        _ => vec![],
    }.join("");

    let page_description = include_str!("../../templates/page-description.html")
    .replace("$$DESCR$$", &page_descr);

    let mut index_body_html = include_str!("../../templates/index-body.html").to_string();
    index_body_html = index_body_html.replace("<!-- SECTIONS -->", &render_index_first_section(lang, tags, articles));
    index_body_html = index_body_html.replace("<!-- SECTION_EXTRA -->", &render_other_index_sections(lang, tags, articles)?);
    index_body_html = index_body_html.replace("<!-- SEARCHBAR -->", &searchbar_html);

    let title_id = format!("{lang}-index");
    let mut index_html = include_str!("../../templates/index-template.html").to_string();
    index_html = index_html.replace("<!-- BODY_ABSTRACT -->", &index_body_html);
    index_html = index_html.replace("<!-- PAGE_DESCRIPTION -->", &page_description);
    index_html = index_html.replace("<!-- SVG_LOGO_INLINE -->", logo_svg);
    index_html = index_html.replace("<!-- HEAD_TEMPLATE_HTML -->", &head(&a, lang, &title_id));
    index_html = index_html.replace("<!-- PAGE_HELP -->", &page_help);
    index_html = index_html.replace("<!-- HEADER_NAVIGATION -->", &header_navigation(lang, false));
    index_html = index_html.replace("<!-- MULTILANG_TAGS -->", multilang);
    index_html = index_html.replace("$$SKIP_TO_MAIN_CONTENT$$", "Skip to main content");
    index_html = index_html.replace("$$TITLE$$", &title);
    index_html = index_html.replace("$$DESCRIPTION$$", &description);
    index_html = index_html.replace("$$TITLE_ID$$", &title_id);
    index_html = index_html.replace("$$LANG$$", lang);
    index_html = index_html.replace("$$SLUG$$", "");
    index_html = index_html.replace("$$SELECT_FAITH$$", select_faith);
    index_html = index_html.replace("$$ROOT_HREF$$", get_root_href());
    index_html = index_html.replace("$$PAGE_HREF$$", &(get_root_href().to_string() + "/" + lang));
    index_html = index_html.replace("<link rel=\"preload\" href=\"/static/img/logo/logo-smooth.svg\" as=\"image\">", "<link rel=\"preload\" href=\"/static/img/ornament/sun-verginasun-black.svg\" as=\"image\">");
    index_html = index_html.replace("<link rel=\"preload\" href=\"/static/font/ssfp/ssp/SourceSansPro-BASIC-Regular.ttf\" as=\"font\" type=\"font/ttf\" crossorigin>", "");
    index_html = index_html.replace("<link rel=\"preload\" href=\"/static/font/quivira/Quivira-SUBSETTED.ttf\" as=\"font\" type=\"font/ttf\" crossorigin>", "");
    
    Ok(index_html)
}

fn minify(input: &str) -> Vec<u8> {
    let min = minify_html::Cfg {
        do_not_minify_doctype: true,
        ensure_spec_compliant_unquoted_attribute_values: false,
        keep_closing_tags: true,
        keep_html_and_head_opening_tags: true,
        keep_spaces_between_attributes: true,
        keep_comments: false,
        keep_input_type_text_attr: true,
        keep_ssi_comments: false,
        preserve_brace_template_syntax: true,
        preserve_chevron_percent_template_syntax: true,
        minify_css: true,
        minify_js: true,
        remove_bangs: true,
        remove_processing_instructions: true,
    };
    minify_html::minify(input.as_bytes(), &min)
}

fn main() -> Result<(), String> {

    // Setup 
    let mut cwd = std::env::current_dir()
        .map_err(|e| e.to_string())?;
    
    while !cwd.join("articles").is_dir() {
        cwd = cwd.parent().ok_or("cannot find /articles dir in current path")?.to_path_buf();
    }

    let authors = std::fs::read_to_string(&cwd.join("authors.json"))
    .map_err(|e| e.to_string())?;
    let authors_map = read_authors_map(&authors);

    let tags = std::fs::read_to_string(&cwd.join("tags.json"))
    .map_err(|e| e.to_string())?;
    let tags = tags_map(&tags)?;

    let dir = cwd.join("articles");

    // Load, parse and analyze articles
    let articles = load_articles(&dir)?;
    let vectorized = articles.vectorize();
    let analyzed = vectorized.analyze();

    // Render and write articles
    let mut articles_by_tag = ArticlesByTag::default();
    let mut articles_by_date = ArticlesByDate::default();

    for (lang, articles) in analyzed.map.iter() {
        for (slug, a) in articles {
            
            let s = article2html(
                &lang, 
                &slug, 
                &a, 
                true, 
                &mut articles_by_tag, 
                &mut articles_by_date,
                &authors_map,
            );

            match s {
                Ok(s) => {
                    let path = cwd.join(lang);
                    let _ = std::fs::create_dir_all(&path);
                    let _ = std::fs::write(path.join(slug.to_string() + ".html"), &minify(&s));
                },
                Err(e) if e.is_empty() => { },
                Err(q) => return Err(q),
            }
        }
    }

    // Write author pages
    let author_pages = render_page_author_pages(&analyzed, &authors_map)?;
    for (lang, authors) in author_pages.iter() {
        let _ = std::fs::create_dir_all(cwd.join(&lang).join("author"));
        for (a, v) in authors {
            let _ = std::fs::write(cwd.join(&lang).join("author").join(&format!("{a}.html")), &minify(&v));
        }
    }

    // Generate search index
    let si = generate_search_index(&analyzed);
    for (lang, si) in si.iter() {
        let json = serde_json::to_string(&si).unwrap_or_default();
        let _ = std::fs::write(cwd.join(lang).join("index.json"), json);
    }

    // Write special pages
    let langs = analyzed.map.keys().cloned().collect::<Vec<_>>();
    for l in langs.iter() {
        let sp = get_special_pages(&l, &tags, &articles_by_tag, &articles_by_date)?;
        for s in sp.iter() {
            let (path, html) = special2html(&l, s);
            let _ = std::fs::write(cwd.join(l).join(path), &minify(&html));
        }
    }

    // Write index + /search pages
    let si = search_html(&analyzed);
    for (lang, (_searchbar_html, search_html, search_js)) in si.iter() {
        let _ = std::fs::write(cwd.join(lang).join("search.js"), search_js);
        let _ = std::fs::write(cwd.join(lang).join("search.html"), &minify(&search_html));
        let index_html = render_index_html(lang, &analyzed, &tags, &si)?;
        let _ = std::fs::write(cwd.join(&format!("{lang}.html")), &minify(&index_html));
        let _ = std::fs::write(cwd.join(lang).join("head.js"), &strip_comments(include_str!("../../static/js/head.js")));
    }

    // Write gitignore
    let _ = std::fs::write(cwd.join(".gitignore"), generate_gitignore(&articles));

    Ok(())
}
