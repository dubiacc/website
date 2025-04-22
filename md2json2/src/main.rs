use rosary::RosaryMysteries;
use rosary::RosaryTemplates;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

mod langtrain;
mod resistance;
mod rosary;

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

impl AnalyzedArticles {
    pub fn get_chars(&self) -> BTreeSet<char> {
        self.map
            .values()
            .flat_map(|v| v.values().flat_map(|p| p.get_chars()))
            .collect()
    }
}

#[derive(Debug, Default)]
struct ParsedArticle {
    src: String,
    title: String,
    date: String,
    tags: Vec<String>,
    authors: Vec<String>,
    sha256: String,
    img: Option<Image>,
    summary: Vec<Paragraph>,
    article_abstract: Vec<Paragraph>,
    sections: Vec<ArticleSection>,
    footnotes: Vec<Footnote>,
}

impl ParsedArticle {
    pub fn get_bibliography(&self) -> Vec<Link> {
        let mut s = self
            .sections
            .iter()
            .flat_map(|l| {
                l.pars.iter().flat_map(|p| match p {
                    Paragraph::Sentence { s } => s
                        .iter()
                        .filter_map(|s| match s {
                            SentenceItem::Link { l } => Some(l.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                    Paragraph::Quote { q } => {
                        let mut links = q.get_links();
                        for l in q.quote.iter() {
                            match l {
                                Paragraph::Sentence { s } => {
                                    for t in s.iter() {
                                        if let SentenceItem::Link { l } = t {
                                            // TODO: add titles to link!
                                            links.push(l.clone());
                                        }
                                    }
                                }
                                Paragraph::Quote { q } => {
                                    links.extend(q.get_links().into_iter());
                                }
                                _ => {}
                            }
                        }
                        links
                    }
                    Paragraph::Image { i } => Vec::new(),
                })
            })
            .collect::<Vec<_>>();

        s.sort();
        s.dedup();

        s
    }
}

impl VectorizedArticles {
    pub fn analyze(&self) -> AnalyzedArticles {
        AnalyzedArticles {
            map: self
                .map
                .iter()
                .map(|(lang, v)| {
                    (
                        lang.clone(),
                        v.iter()
                            .map(|(slug, vectorized)| {
                                let similar = get_similar_articles(vectorized, slug, v);

                                // gather all links of other sites that link here
                                let backlinks = self
                                    .map
                                    .iter()
                                    .flat_map(|(lang2, v2)| {
                                        v2.iter().filter_map(move |(slug2, vectorized2)| {
                                            if lang2 != lang {
                                                return None;
                                            }

                                            if slug2 == slug {
                                                return None; // don't self-link
                                            }

                                            let needle = format!("{lang2}/{slug2}");
                                            if vectorized2.parsed.src.contains(&needle) {
                                                Some(SectionLink {
                                                    title: vectorized2.parsed.title.clone(),
                                                    slug: slug2.clone(),
                                                    id: None,
                                                })
                                            } else {
                                                None
                                            }
                                        })
                                    })
                                    .collect();

                                (
                                    slug.clone(),
                                    ParsedArticleAnalyzed {
                                        title: vectorized.parsed.title.clone(),
                                        date: vectorized.parsed.date.clone(),
                                        tags: vectorized.parsed.tags.clone(),
                                        authors: vectorized.parsed.authors.clone(),
                                        sha256: vectorized.parsed.sha256.clone(),
                                        img: vectorized.parsed.img.clone(),
                                        subtitle: vectorized.parsed.summary.clone(),
                                        summary: vectorized.parsed.article_abstract.clone(),
                                        sections: vectorized.parsed.sections.clone(),
                                        similar: similar,
                                        backlinks: backlinks,
                                        bibliography: vectorized.parsed.get_bibliography(),
                                        footnotes: vectorized.parsed.footnotes.clone(),
                                    },
                                )
                            })
                            .collect(),
                    )
                })
                .collect(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct ParsedArticleAnalyzed {
    // title of the article
    title: String,
    // date of the article (yyyy-mm-dd)
    date: String,
    // tags of the article
    tags: Vec<String>,
    // authors
    authors: Vec<String>,
    // sha256 hash of the article contents
    sha256: String,
    // social media preview image
    img: Option<Image>,
    // subtitle: displayed below the title
    subtitle: Vec<Paragraph>,
    // summary: displayed as the first section
    summary: Vec<Paragraph>,
    // sections of the article
    sections: Vec<ArticleSection>,
    // similar articles to this one based on word matching
    similar: Vec<SectionLink>,
    // Articles that link to this link
    backlinks: Vec<SectionLink>,
    // all links used in this page
    bibliography: Vec<Link>,
    // footnote annotations
    footnotes: Vec<Footnote>,
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize)]
struct Footnote {
    pub id: String,
    pub text: Vec<SentenceItem>,
}

impl Footnote {
    pub fn get_chars(&self) -> Vec<char> {
        let mut v = self.id.chars().collect::<Vec<_>>();
        v.extend(si2text(&self.text).chars());
        v
    }
}

fn parse_footnote(s: &str) -> Option<Footnote> {
    if !(s.trim().starts_with("[^") && s.contains("]:")) {
        return None;
    }

    let (id, rest) = s.split_once(":")?;
    let id = id.replace("[^", "").replace("]", "").trim().to_string();
    let text = Sentence::new(rest.trim()).items;
    Some(Footnote { id, text })
}

#[test]
fn test_parse_footnote() {
    let s = "[^1]: Some text";
    assert_eq!(
        parse_footnote(s).unwrap(),
        Footnote {
            id: "1".to_string(),
            text: vec![SentenceItem::Text {
                text: "Some text".to_string()
            },]
        }
    );

    let s = "[^ref]: Some text with [a link](https://example.com).";
    let p = parse_footnote(s).unwrap();
    assert_eq!(
        p,
        Footnote {
            id: "ref".to_string(),
            text: vec![
                SentenceItem::Text {
                    text: "Some text with ".to_string()
                },
                SentenceItem::Link {
                    l: Link {
                        text: "a link".to_string(),
                        href: "https://example.com".to_string(),
                        title: "a link".to_string(),
                        id: uuid("https://example.com"),
                    }
                },
                SentenceItem::Text {
                    text: ".".to_string()
                },
            ]
        }
    );
}

impl ParsedArticleAnalyzed {
    pub fn is_prayer(&self) -> bool {
        self.tags.iter().any(|s| s == "gebet" || s == "prayer")
    }
    pub fn get_chars(&self) -> Vec<char> {
        let mut c = self.title.chars().collect::<Vec<_>>();
        c.extend(self.date.chars());
        c.extend(self.tags.iter().flat_map(|q| q.chars()));
        c.extend(self.subtitle.iter().flat_map(|s| s.get_chars()));
        c.extend(self.summary.iter().flat_map(|s| s.get_chars()));
        c.extend(self.sections.iter().flat_map(|s| s.title.chars()));
        c.extend(
            self.sections
                .iter()
                .flat_map(|s| s.pars.iter().flat_map(|r| r.get_chars())),
        );
        c.extend(self.footnotes.iter().flat_map(|s| s.get_chars()));
        c
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
    Image { i: Image },
}

impl Paragraph {
    pub fn as_sentence(&self) -> Option<&[SentenceItem]> {
        if let Paragraph::Sentence { s } = self {
            Some(s)
        } else {
            None
        }
    }

    pub fn word_count(&self) -> usize {
        match self {
            Paragraph::Sentence { s } => s
                .iter()
                .map(|z| match z {
                    SentenceItem::Text { text } => text.split_whitespace().count() + 1,
                    SentenceItem::Link { l } => l.text.split_whitespace().count() + 1,
                    SentenceItem::Footnote { .. } => 0,
                })
                .sum(),
            Paragraph::Quote { q } => q.quote.iter().map(|p| p.word_count()).sum(),
            Paragraph::Image { i } => 0,
        }
    }

    pub fn get_chars(&self) -> Vec<char> {
        match self {
            Paragraph::Sentence { s } => s
                .iter()
                .flat_map(|z| match z {
                    SentenceItem::Text { text } => text.chars().collect::<Vec<_>>(),
                    SentenceItem::Link { l } => l.text.chars().collect::<Vec<_>>(),
                    SentenceItem::Footnote { id } => id.chars().collect::<Vec<_>>(),
                })
                .collect::<Vec<_>>(),
            Paragraph::Quote { q } => {
                let mut p = q
                    .quote
                    .iter()
                    .flat_map(|p| p.get_chars())
                    .collect::<Vec<_>>();
                p.extend(q.title.chars());
                p.extend(q.author.clone().unwrap_or_default().text.chars());
                p.extend(q.source.clone().unwrap_or_default().text.chars());
                p
            }
            Paragraph::Image { i } => i.title.chars().collect(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
struct Sentence {
    items: Vec<SentenceItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "t", content = "d", rename_all = "lowercase")]
enum SentenceItem {
    Text { text: String },
    Link { l: Link },
    Footnote { id: String },
}

impl SentenceItem {
    pub fn text(&self) -> Option<&String> {
        if let SentenceItem::Text { text } = self {
            Some(text)
        } else {
            None
        }
    }

    pub fn is_link(&self) -> bool {
        match self {
            SentenceItem::Link { l } => true,
            _ => false,
        }
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
    quote: Vec<Paragraph>,
    author: Option<Link>,
    source: Option<Link>,
}

impl Quote {
    pub fn get_links(&self) -> Vec<Link> {
        let mut v = Vec::new();

        if let Some(l) = self.author.as_ref() {
            v.push(l.clone());
        }

        if let Some(l) = self.source.as_ref() {
            v.push(l.clone());
        }

        v
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct Link {
    text: String,
    href: String,
    title: String,
    id: String,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
struct Image {
    href: String,
    alt: String,
    title: String,
    inline: Option<ImageAlignment>,
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
enum ImageAlignment {
    FullWidth,
    Left(usize),
    Right(usize),
}

impl Default for ImageAlignment {
    fn default() -> Self {
        ImageAlignment::Right(200)
    }
}

fn parse_image_align(s: &str) -> Option<ImageAlignment> {
    let al = "align-left(";
    let ar = "align-right(";

    if s.contains("full-width") {
        Some(ImageAlignment::FullWidth)
    } else if s.trim().starts_with(al) {
        let chars = s
            .trim()
            .chars()
            .skip(ar.len())
            .take_while(|c| c.is_ascii_alphanumeric() && c.is_numeric())
            .collect::<String>();

        let width = chars.parse::<usize>().ok()?;

        Some(ImageAlignment::Right(width))
    } else if s.trim().starts_with(ar) {
        let chars = s
            .trim()
            .chars()
            .skip(ar.len())
            .take_while(|c| c.is_ascii_alphanumeric() && c.is_numeric())
            .collect::<String>();

        let width = chars.parse::<usize>().ok()?;

        Some(ImageAlignment::Right(width))
    } else {
        None
    }
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
    lines
        .split(|s| s.is_empty())
        .map(|q| q.to_vec())
        .collect::<Vec<Vec<_>>>()
        .iter()
        .filter(|s| !s.is_empty())
        .map(|sp| parse_paragraph(&sp.join("\r\n")))
        .collect()
}

#[cfg(feature = "external")]
fn sha256(s: &str) -> String {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let result = hasher.finalize();
    base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(result)
}

fn gather_footnotes(l: &[&str]) -> (Vec<Footnote>, BTreeSet<usize>) {
    let mut to_ignore = BTreeSet::new();
    let mut target = Vec::new();
    for (i, l) in l.iter().enumerate() {
        if let Some(f) = parse_footnote(&l) {
            to_ignore.insert(i);
            target.push(f);
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

    let config = serde_json::from_str::<Config>(&codeblock.join("\r\n")).unwrap_or_default();

    (config, to_ignore)
}

fn parse_article(s: &str) -> ParsedArticle {
    let lines = s.lines().collect::<Vec<_>>();
    let (title_line, title) = lines
        .iter()
        .enumerate()
        .filter(|(_, s)| s.starts_with("# "))
        .map(|(i, q)| (i, q.replace("# ", "").trim().to_string()))
        .next()
        .unwrap_or((0, String::new()));

    let sha256 = sha256(&s);

    let (config, lines_to_ignore) = extract_config(&lines);

    let lines_before_heading = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            if lines_to_ignore.contains(&i) || i >= title_line {
                None
            } else {
                Some(*l)
            }
        })
        .collect::<Vec<_>>();

    let lines_after_heading = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            if lines_to_ignore.contains(&i) || i <= title_line {
                None
            } else {
                Some(*l)
            }
        })
        .collect::<Vec<_>>();

    let article_abstract = lines_after_heading
        .iter()
        .take_while(|s| !s.contains("# "))
        .cloned()
        .collect::<Vec<_>>();

    let lines_after_heading = lines_after_heading[article_abstract.len()..].to_vec();
    let (footnotes, footnote_lines) = gather_footnotes(&lines_after_heading);
    let lines_after_heading = lines_after_heading
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            if footnote_lines.contains(&i) {
                None
            } else {
                Some(s)
            }
        })
        .collect::<Vec<_>>();

    let mut sections = lines_after_heading
        .iter()
        .enumerate()
        .filter_map(|(i, s)| if s.contains("# ") { Some(i) } else { None })
        .collect::<Vec<_>>();
    sections.push(lines_after_heading.len());

    let sections = sections
        .windows(2)
        .filter_map(|s| {
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
        })
        .collect::<Vec<_>>();

    ParsedArticle {
        src: s.to_string(),
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
        let is_question = s
            .lines()
            .filter(|q| q.starts_with("# "))
            .any(|q| q.trim().ends_with("?"));
        let is_prayer = s.lines().any(|q| q.trim() == "Amen.")
            || s.lines()
                .any(|s| s.contains("tags") && (s.contains("gebet") || s.contains("prayer")));
        if is_prayer {
            ArticleType::Prayer
        } else if is_question {
            ArticleType::Question
        } else {
            ArticleType::Tract
        }
    }
}

#[test]
fn test_parse_quote() {
    let s = "
        > Test
        >
        > > Indent
        > > IndentLine2
        >
        > Continued
    "
    .lines()
    .map(|s| s.trim())
    .collect::<Vec<_>>()
    .join("\r\n");

    let q = Quote::new(&s).unwrap();

    assert_eq!(
        q,
        Quote {
            title: String::new(),
            quote: vec![
                Paragraph::Sentence {
                    s: vec![SentenceItem::Text {
                        text: "Test".to_string()
                    }]
                },
                Paragraph::Quote {
                    q: Quote {
                        title: String::new(),
                        quote: vec![Paragraph::Sentence {
                            s: vec![SentenceItem::Text {
                                text: "Indent IndentLine2".to_string()
                            }]
                        },],
                        author: None,
                        source: None,
                    }
                },
                Paragraph::Sentence {
                    s: vec![SentenceItem::Text {
                        text: "Continued".to_string()
                    }]
                },
            ],
            author: None,
            source: None,
        }
    );
}

impl Quote {
    fn new(s: &str) -> Option<Self> {
        let mut lines = s
            .trim()
            .lines()
            .map(|l| l.trim())
            .filter(|l| l.trim().starts_with(">"))
            .map(|l| l.replacen(">", "", 1).trim().to_string())
            .collect::<Vec<_>>();

        if lines.is_empty() {
            return None;
        }

        let title = lines.iter().find(|s| s.starts_with("**")).cloned();

        if let Some(t) = title.as_deref() {
            lines.retain(|l| l.as_str() != t);
        }

        let title = title.map(|l| l.replace("**", "")).unwrap_or_default();

        let author_line = lines
            .iter()
            .find(|s| s.trim().starts_with("--") || s.trim().starts_with("—-"))
            .cloned();

        if let Some(t) = author_line.as_deref() {
            lines.retain(|l| l.as_str() != t);
        }

        let author_line = author_line
            .map(|s| {
                s.replacen("--", "", 1)
                    .replacen("—-", "", 1)
                    .trim()
                    .to_string()
            })
            .unwrap_or_default();

        let mut author = None;
        let mut source = None;
        let mut author_line = &author_line[..];

        if let Some((next_link, to_delete)) = take_next_link(&author_line) {
            author = Some(next_link);
            author_line = &author_line[to_delete..];
        }

        let next_link_start =
            author_line
                .char_indices()
                .find_map(|(idx, c)| if c == '[' { Some(idx) } else { None });

        if let Some(nls) = next_link_start {
            author_line = &author_line[nls..];
        }

        if let Some((next_link, _)) = take_next_link(&author_line) {
            source = Some(next_link);
        }

        let lines = lines
            .iter()
            .filter(|s| !s.starts_with("**"))
            .filter(|s| !s.starts_with("--"))
            .cloned()
            .collect::<Vec<_>>();

        let mut quote = lines
            .split(|s| s.trim().is_empty())
            .map(|q| q.to_vec())
            .filter(|s| !s.iter().all(|q| q.is_empty()))
            .collect::<Vec<Vec<String>>>();

        if let Some(fl) = quote.first_mut().and_then(|s| s.first_mut()) {
            if fl.trim().starts_with("\"") {
                *fl = fl.replacen("\"", "", 1);
            } else if fl.trim().starts_with("'") {
                *fl = fl.replacen("'", "", 1);
            } else if fl.trim().starts_with("`") {
                *fl = fl.replacen("`", "", 1);
            }
        }

        if let Some(fl) = quote
            .last_mut()
            .and_then(|s: &mut Vec<String>| s.last_mut())
        {
            if fl.trim().ends_with("\"") {
                *fl = fl.replacen("\"", "", 1);
            } else if fl.trim().ends_with("'") {
                *fl = fl.replacen("'", "", 1);
            } else if fl.trim().ends_with("`") {
                *fl = fl.replacen("`", "", 1);
            }
        }

        let quote = quote
            .iter()
            .map(|q| parse_paragraph(&q.join("\r\n")))
            .collect::<Vec<_>>();

        let q = Quote {
            title,
            quote,
            author,
            source,
        };

        Some(q)
    }
}

// Given a string, returns the extracted link + number of bytes to be consumed
fn take_next_link(s: &str) -> Option<(Link, usize)> {
    let s = s.trim();
    if !s.starts_with("[") {
        return None;
    }

    let mut it_str = Vec::new();
    let mut it = s.chars().peekable();
    it.next(); // remove first '['
    while let Some(s) = it.next() {
        let peek = it.peek();
        match (s, peek) {
            (']', Some('(')) => break,
            (']', _) | ('[', _) => return None,
            _ => it_str.push(s),
        }
    }

    let id = it_str.into_iter().collect::<String>();
    let s = s.replacen(&("[".to_string() + &id + "]("), "", 1);

    let mut open_br_count = 1;
    let mut link = Vec::new();
    for c in s.chars() {
        if c == '(' {
            open_br_count += 1;
        } else if c == ')' {
            open_br_count -= 1;
        }

        if open_br_count == 0 {
            break;
        }

        link.push(c);
    }

    let link = link.into_iter().collect::<String>();
    let bytes = id.len() + 4 + link.len();

    let text = id.clone();
    let mut href = link.to_string();
    let mut title = id.clone();

    if link.contains(" ") {
        let (new_href, new_title) = link.split_once(' ')?;
        href = new_href.to_string();
        title = new_title.to_string();
    }

    if href.starts_with("/") {
        href = get_root_href().to_string() + &href;
    }

    let l = Link {
        text,
        href: href.clone(),
        title,
        id: uuid(&href),
    };
    Some((l, bytes))
}

fn uuid(seed: &str) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(seed.as_bytes());
    let f = hasher.finalize();
    let b = [
        f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8], f[9], f[10], f[11], f[12], f[13],
        f[14], f[15],
    ];
    let uuid = uuid::Builder::from_sha1_bytes(b).into_uuid();
    short_uuid::ShortUuid::from_uuid(&uuid).to_string()
}

#[test]
fn test_quote_2() {
    let s = "
        > Wenn ein Mann eine Jungfrau trifft, die nicht verlobt ist
        > 
        > —- [5. Mose 22,28-29](https://k-bibel.de/ARN/Deuteronomium22#28-29)
    ";

    assert_eq!(
        Quote::new(s),
        Some(Quote {
            title: "".to_string(),
            quote: vec![Paragraph::Sentence {
                s: vec![SentenceItem::Text {
                    text: "Wenn ein Mann eine Jungfrau trifft, die nicht verlobt ist".to_string()
                }]
            },],
            author: Some(Link {
                text: "5. Mose 22,28-29".to_string(),
                href: "https://k-bibel.de/ARN/Deuteronomium22#28-29".to_string(),
                title: "5. Mose 22,28-29".to_string(),
                id: uuid("https://k-bibel.de/ARN/Deuteronomium22#28-29"),
            }),
            source: None,
        })
    )
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

    let q = Quote::new(s).unwrap();
    assert_eq!(
        q,
        Quote {
            title: "Heading".to_string(),
            quote: vec![
                Paragraph::Sentence {
                    s: vec![SentenceItem::Text {
                        text: "LineA LineB LineC".to_string()
                    }]
                },
                Paragraph::Sentence {
                    s: vec![SentenceItem::Text {
                        text: "LineD LineE".to_string()
                    }]
                },
            ],
            author: Some(Link {
                text: "Test".to_string(),
                href: "https://wikipedia.org/Test".to_string(),
                title: "Test".to_string(),
                id: q.author.clone().unwrap().id,
            }),
            source: Some(Link {
                text: "de juiribus".to_string(),
                href: "test.pdf".to_string(),
                title: "de juiribus".to_string(),
                id: q.source.clone().unwrap().id,
            }),
        }
    )
}

// parses the footnote from a "[^note]" text
fn parse_footnote_maintext(s: &str) -> Option<(String, usize)> {
    if !s.trim().starts_with("[^") {
        return None;
    }

    let end = s
        .char_indices()
        .find_map(|(idx, c)| if c == ']' { Some(idx) } else { None })?;

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
                                text: cur_sentence
                                    .iter()
                                    .cloned()
                                    .collect::<String>()
                                    .lines()
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            });
                        }
                        items.push(SentenceItem::Footnote { id: footnote_id });
                        cur_sentence.clear();
                        for _ in 0..chars_to_skip.saturating_sub(1) {
                            let _ = iter.next();
                        }
                    }
                    None => {
                        cur_sentence.push(c);
                    }
                },
                ('[', _) => match take_next_link(&s[idx..]) {
                    Some((link, chars_to_skip)) => {
                        if !cur_sentence.is_empty() {
                            items.push(SentenceItem::Text {
                                text: cur_sentence
                                    .iter()
                                    .cloned()
                                    .collect::<String>()
                                    .lines()
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            });
                        }
                        items.push(SentenceItem::Link { l: link });
                        cur_sentence.clear();
                        for _ in 0..chars_to_skip.saturating_sub(1) {
                            let _ = iter.next();
                        }
                    }
                    None => {
                        cur_sentence.push(c);
                    }
                },
                _ => {
                    cur_sentence.push(c);
                }
            }
        }

        if !cur_sentence.is_empty() {
            items.push(SentenceItem::Text {
                text: cur_sentence
                    .iter()
                    .cloned()
                    .collect::<String>()
                    .lines()
                    .collect::<Vec<_>>()
                    .join(" "),
            });
        }

        Self { items }
    }
}

#[test]
fn test_sentence() {
    let s = "This is a sentence with a footnote[^15] and a [link](url).";
    assert_eq!(
        Sentence::new(s),
        Sentence {
            items: vec![
                SentenceItem::Text {
                    text: "This is a sentence with a footnote".to_string()
                },
                SentenceItem::Footnote {
                    id: "15".to_string()
                },
                SentenceItem::Text {
                    text: " and a ".to_string()
                },
                SentenceItem::Link {
                    l: Link {
                        text: "link".to_string(),
                        href: "url".to_string(),
                        title: "link".to_string(),
                        id: uuid("url"),
                    }
                },
                SentenceItem::Text {
                    text: ".".to_string()
                },
            ]
        }
    )
}

impl Image {
    pub fn new(s: &str) -> Option<Self> {
        let mut s = s.trim().to_string();
        if s.starts_with("![") {
            s = s.replacen("![", "", 1);
        } else {
            return None;
        }

        let (alt, rest) = s.split_once("](")?;
        let rest = if rest.contains(")") {
            rest.split(")").next()?.to_string()
        } else {
            return None;
        };

        let (alt, inline) = if alt.contains("::") {
            let (a, i) = alt.split_once("::").unwrap();
            let im = parse_image_align(i);
            (a.trim().to_string(), im)
        } else {
            (alt.trim().to_string(), None)
        };

        let href = rest.split_whitespace().nth(0)?.to_string();
        let title = rest
            .split_whitespace()
            .nth(1)
            .map(|s| s.trim().replace("\"", "").replace("'", "").replace("`", ""))
            .unwrap_or(alt.clone());

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
    assert_eq!(
        Image::new(s),
        Some(Image {
            href: "Isolated.png".to_string(),
            alt: "alt text".to_string(),
            title: "Title".to_string(),
            inline: None,
        })
    );

    let s = "![alt text](Isolated.png)";
    assert_eq!(
        Image::new(s),
        Some(Image {
            href: "Isolated.png".to_string(),
            alt: "alt text".to_string(),
            title: "alt text".to_string(),
            inline: None,
        })
    );

    let s = "![Test)";
    assert_eq!(Image::new(s), None);
}

impl LoadedArticles {
    pub fn vectorize(&self) -> VectorizedArticles {
        fn get_words_of_article(s: &str) -> Vec<&str> {
            s.split_whitespace()
                .filter_map(|s| {
                    if s.contains("[") || s.contains("]") || s.len() < 3 {
                        None
                    } else {
                        Some(s)
                    }
                })
                .collect()
        }

        VectorizedArticles {
            map: self
                .langs
                .iter()
                .map(|(k, v)| {
                    let all_words = v
                        .values()
                        .flat_map(|c| get_words_of_article(c))
                        .collect::<BTreeSet<_>>();
                    let all_words_indexed = all_words
                        .iter()
                        .enumerate()
                        .map(|(i, s)| (*s, i))
                        .collect::<BTreeMap<_, _>>();

                    (
                        k.clone(),
                        v.iter()
                            .map(|(k, v2)| {
                                let embedding = get_words_of_article(v2)
                                    .into_iter()
                                    .filter_map(|q| all_words_indexed.get(q).copied())
                                    .collect();

                                let atype = ArticleType::new(v2);

                                (
                                    k.clone(),
                                    VectorizedArticle {
                                        words: embedding,
                                        atype: atype,
                                        parsed: parse_article(v2),
                                    },
                                )
                            })
                            .collect(),
                    )
                })
                .collect(),
        }
    }
}

/// return similar articles based on string distance for article N
#[cfg(feature = "external")]
fn get_similar_articles(
    s: &VectorizedArticle,
    id: &str,
    map: &BTreeMap<String, VectorizedArticle>,
) -> Vec<SectionLink> {
    let (article_src, article_type) = (&s.words, s.atype);

    let mut target = Vec::new();
    for (other_key, other) in map.iter() {
        if other_key == id {
            continue;
        }

        let penalty = match (article_type, other.atype) {
            (ArticleType::Prayer, ArticleType::Prayer)
            | (ArticleType::Tract, ArticleType::Tract)
            | (ArticleType::Question, ArticleType::Question) => 0,

            (ArticleType::Prayer, _) | (_, ArticleType::Prayer) => continue,

            _ => 10000,
        };

        let dst = strsim::generic_damerau_levenshtein(article_src, &other.words) + penalty;

        target.push((dst, other_key));
    }

    target.sort_by(|a, b| ((a.0) as usize).cmp(&((b.0) as usize)));

    target
        .into_iter()
        .filter_map(|s| {
            Some(SectionLink {
                slug: s.1.clone(),
                title: map.get(s.1)?.parsed.title.clone(),
                id: None,
            })
        })
        .take(10)
        .collect()
}

#[cfg(feature = "external")]
fn load_articles(dir: &Path) -> Result<LoadedArticles, String> {
    let entries = walkdir::WalkDir::new(dir)
        .max_depth(5)
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.map_err(|e| e.to_string()).ok()?;
            let entry = entry.path();
            if entry.file_name().and_then(|s| s.to_str()) == Some("index.md") {
                let name = entry.parent()?;
                let lang = name.parent()?;
                let contents = std::fs::read_to_string(&entry).ok()?;

                Some((
                    lang.file_name()?.to_str()?.to_string(),
                    name.file_name()?.to_str()?.to_string(),
                    contents,
                ))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let mut langs = BTreeMap::new();
    for (lang, id, contents) in entries {
        langs
            .entry(lang)
            .or_insert_with(|| BTreeMap::default())
            .insert(id, contents);
    }

    Ok(LoadedArticles { langs })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct SectionLink {
    id: Option<String>,
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
type ArticlesByDate =
    BTreeMap<Lang, BTreeMap<Year, BTreeMap<Month, BTreeMap<Day, Vec<SectionLink>>>>>;

fn is_prod() -> bool {
    std::env::args().any(|a| a.contains("production"))
}

fn get_root_href() -> &'static str {
    if is_prod() {
        "https://dubia.cc"
    } else {
        "http://localhost:8080"
    }
}

fn gen_serviceworker_js(cwd: &Path, articles: &AnalyzedArticles, meta: &MetaJson) -> String {
    let mut s = include_str!("../../templates/sw.js").to_string();
    s = s.replace(
        "workbox.precaching.precacheAndRoute([]);",
        &gen_sw_paths(cwd, articles, meta),
    );
    s = s.replace(
        "importScripts('/static/js/workbox-sw.js');",
        include_str!("../../static/js/workbox-sw.js"),
    );
    s
}

fn gen_sw_paths(cwd: &Path, articles: &AnalyzedArticles, meta: &MetaJson) -> String {
    let mut site_revision = Vec::new();
    let mut a = articles
        .map
        .iter()
        .flat_map(|(lang, v)| {
            let mut q = v
                .iter()
                .map(move |(slug, a)| {
                    format!(
                        "    {{ url: '/{lang}/{slug}.html', revision: '{}' }}",
                        a.sha256
                    )
                })
                .collect::<Vec<_>>();

            q.extend(v.iter().map(|(slug, a)| {
                site_revision.push(a.sha256.clone());
                format!(
                    "    {{ url: '/articles/{lang}/{slug}/index.md', revision: '{}' }}",
                    a.sha256
                )
            }));

            q
        })
        .collect::<Vec<_>>();

    let site_revision = sha256(&site_revision.join(" "));
    for l in articles.map.keys() {
        a.push(format!(
            "    {{ url: '/{l}.html', revision: '{site_revision}' }}"
        ));
        a.push(format!(
            "    {{ url: '/{l}/search.js', revision: '{site_revision}' }}"
        ));
        a.push(format!(
            "    {{ url: '/{l}/search.html', revision: '{site_revision}' }}"
        ));
        a.push(format!(
            "    {{ url: '/{l}/index.json', revision: '{site_revision}' }}"
        ));
    }

    for author in meta.authors.keys() {
        for lang in articles.map.keys() {
            let q = author.replace(":", "-");
            let meta_v = sha256(&serde_json::to_string(&meta).unwrap_or_default());
            a.push(format!(
                "    {{ url: '/{lang}/author/{q}.html', revision: '{meta_v}' }}"
            ));
        }
    }

    for lang in articles.map.keys() {
        for sp in get_special_pages(
            lang,
            meta,
            &ArticlesByTag::default(),
            &ArticlesByDate::default(),
        )
        .unwrap_or_default()
        {
            a.push(format!(
                "    {{ url: '/{lang}/{}', revision: '{site_revision}' }}",
                sp.filepath
            ));
        }
    }

    // add all images from articles
    let l = walkdir::WalkDir::new(cwd.join("articles"))
        .max_depth(5)
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.map_err(|e| e.to_string()).ok()?;
            let entry = entry.path();
            let ext = entry.extension().and_then(|s| s.to_str());
            let fname = entry.file_name()?.to_str()?.to_string();
            let last_modified = format!("{:?}", entry.metadata().ok()?.modified().ok()?)
                .chars()
                .filter(|c| c.is_numeric())
                .collect::<String>();

            if ext == Some(".avif") || ext == Some("avif") {
                let slug = entry.parent()?;
                let lang = slug.parent()?;
                let slug = slug.file_name()?.to_str()?.to_string();
                let lang = lang.file_name()?.to_str()?.to_string();
                Some((lang, slug, fname, last_modified))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // /index.html
    a.push(format!(
        "    {{ url: '/index.html', revision: '{}' }}",
        sha256(include_str!("../../index.html"))
    ));

    a.push(format!(
        "    {{ url: '/static/js/head2.js', revision: '{}' }}",
        sha256(include_str!("../../static/js/head2.js"))
    ));

    // author pages
    // special pages

    // fonts
    a.push(format!(
        "    {{ url: '/static/font/ssp/SourceSansPro-BASIC-Semibold.woff2', revision: '1' }}"
    ));
    a.push(format!(
        "    {{ url: '/static/font/ssp/SourceSansPro-BASIC-Bold.woff2', revision: '1' }}"
    ));
    a.push(format!(
        "    {{ url: '/static/font/ssp/SourceSansPro-BASIC-RegularItalic.woff2', revision: '1' }}"
    ));
    a.push(format!(
        "    {{ url: '/static/font/ssp/SourceSansPro-BASIC-Regular.woff2', revision: '1' }}"
    ));
    a.push(format!(
        "    {{ url: '/static/font/ssp/SourceSansPro-BASIC-BoldItalic.woff2', revision: '1' }}"
    ));
    a.push(format!(
        "    {{ url: '/static/font/ssp/SourceSansPro-BASIC-SemiboldItalic.woff2', revision: '1' }}"
    ));
    a.push(format!(
        "    {{ url: '/static/font/ssfp/SourceSerifPro-BASIC-Regular.woff2', revision: '1' }}"
    ));
    a.push(format!("    {{ url: '/static/font/ssfp/SourceSerifPro-BASIC-RegularItalic.woff2', revision: '1' }}"));
    a.push(format!(
        "    {{ url: '/static/font/ssfp/SourceSerifPro-BASIC-Bold.woff2', revision: '1' }}"
    ));
    a.push(format!("    {{ url: '/static/font/ssfp/SourceSerifPro-BASIC-SemiboldItalic.woff2', revision: '1' }}"));
    a.push(format!(
        "    {{ url: '/static/font/ssfp/SourceSerifPro-BASIC-Semibold.woff2', revision: '1' }}"
    ));
    a.push(format!(
        "    {{ url: '/static/font/ssfp/SourceSerifPro-BASIC-BoldItalic.woff2', revision: '1' }}"
    ));

    for i in 'A'..'Z' {
        a.push(format!(
            "    {{ url: '/static/font/kanzlei/Kanzlei-Initialen-{i}.ttf', revision: '1' }}",
        )); // never changes
    }

    a.push(format!(
        "    {{ url: '/static/js/search.js', revision: '{}' }}",
        sha256(include_str!("../../static/js/search.js"))
    ));
    a.push(format!(
        "    {{ url: '/static/js/head.js', revision: '{}' }}",
        sha256(include_str!("../../static/js/head.js"))
    ));
    a.push(format!(
        "    {{ url: '/static/css/head.css', revision: '{}' }}",
        sha256(include_str!("../../static/css/head.css"))
    ));
    a.push(format!(
        "    {{ url: '/static/css/style.css', revision: '{}' }}",
        sha256(include_str!("../../static/css/style.css"))
    ));

    a.push(format!(
        "    {{ url: '/static/img/logo/logo-smooth.svg', revision: '1' }}"
    ));
    a.push(format!(
        "    {{ url: '/static/img/watercolor.avif', revision: '1' }}",
    )); // never changes
    a.push(format!(
        "    {{ url: '/static/img/death.avif', revision: '1' }}",
    )); // never changes
    a.push(format!(
        "    {{ url: '/static/img/icon/icons.svg', revision: '1' }}",
    )); // never changes
    a.push(format!(
        "    {{ url: '/static/img/shop/books.avif', revision: '1' }}",
    )); // never changes
    a.push(format!(
        "    {{ url: '/static/img/shop/donation.avif', revision: '1' }}",
    )); // never changes
    a.push(format!(
        "    {{ url: '/static/img/shop/mensfashion.avif', revision: '1' }}",
    )); // never changes
    a.push(format!(
        "    {{ url: '/static/img/shop/womansfashion.avif', revision: '1' }}",
    )); // never changes
    a.push(format!(
        "    {{ url: '/static/img/shop/rosary.avif', revision: '1' }}",
    )); // never changes
    a.push(format!(
        "    {{ url: '/static/img/ornament/asterism-triplewhitestar.svg', revision: '1' }}",
    )); // never changes
    a.push(format!(
        "    {{ url: '/static/img/ornament/sun-verginasun-black.svg', revision: '1' }}",
    )); // never changes
    a.push(format!("    {{ url: '/static/img/ornament/japanesecrest-tsukinihoshi-dottedmoon.svg', revision: '1' }}", )); // never changes
    a.push(format!(
        "    {{ url: '/static/img/ornament/sequential-nav-icons-arabesque.svg', revision: '1' }}",
    )); // never changes
    a.push(format!(
        "    {{ url: '/static/img/ornament/three-wavy-lines-ornament-right.svg', revision: '1' }}",
    )); // never changes
    a.push(format!(
        "    {{ url: '/static/img/ornament/three-wavy-lines-ornament-left.svg', revision: '1' }}",
    )); // never changes
    a.push(format!(
        "    {{ url: '/static/img/shop/rosary.avif', revision: '1' }}",
    )); // never changes

    a.push(format!(
        "    {{ url: '/static/img/logo/logo-sm-32.avif', revision: '1' }}"
    ));
    a.push(format!(
        "    {{ url: '/static/img/logo/logo-sm-dark-32.avif', revision: '1' }}"
    ));
    a.push(format!("    {{ url: '/death.html', revision: '1' }}"));
    a.push(format!("    {{ url: '/manifest.json', revision: '1' }}"));

    for (lang, slug, image, rev) in l {
        a.push(format!(
            "    {{ url: '/{lang}/{slug}/{image}', revision: '{rev}' }}"
        ));
    }

    format!(
        "workbox.precaching.precacheAndRoute([\r\n{}\r\n]);",
        a.join(",\r\n")
    )
}

fn generate_gitignore(articles: &LoadedArticles, meta: &MetaJson) -> String {
    let mut filenames = BTreeSet::new();
    for lang in articles.langs.keys() {
        filenames.insert(format!("/{lang}"));
        filenames.insert(format!("/{lang}2"));
        filenames.insert(format!("{lang}.html"));
    }
    for lang in meta.strings.keys() {
        filenames.insert(format!("/{lang}"));
        filenames.insert(format!("/{lang}2"));
        filenames.insert(format!("{lang}.html"));
    }
    filenames.insert("/venv".into());
    filenames.insert("*.md.json".into());
    filenames.insert("sw.js".into());
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

fn get_title(lang: &str, a: &ParsedArticleAnalyzed, meta: &MetaJson) -> Result<String, String> {
    if !a.title.trim().is_empty() {
        return Ok(a.title.clone());
    }

    get_string(meta, lang, "index-title")
}

fn si2text(si: &[SentenceItem]) -> String {
    si.iter()
        .map(|s| match s {
            SentenceItem::Footnote { .. } => String::new(),
            SentenceItem::Link { l } => l.text.clone(),
            SentenceItem::Text { text } => text.clone(),
        })
        .collect::<Vec<_>>()
        .join("")
}

fn rv() -> (String, String) {
    let s = "display:inline;color:red;width: 25px;height: 25px;position: relative;top: 7px;margin-right: 10px;";
    let r = format!(
        "<div style='{s}'>{}</div>",
        include_str!("../../templates/response.svg")
    );
    let v = format!(
        "<div style='{s}'>{}</div>",
        include_str!("../../templates/versicle.svg")
    );
    (r, v)
}

fn si2html(si: &[SentenceItem]) -> String {
    let (r, v) = rv();

    let s = si.iter().map(|s| match s {
        SentenceItem::Footnote { id } => format!("<a href='#fn{id}' class='footnote-ref spawns-popup' id='fnref{id}' role='doc-noteref'><sup>{id}</sup></a>"),
        SentenceItem::Link { l } => format!("<a href='{}'>{}</a>", l.href, l.text),
        SentenceItem::Text { text } => text.replace("[R]: ", &r).replace("[V]: ", &v),
    }).collect::<Vec<_>>().join("");

    format!("<p class='first-graf'>{s}</p>")
}

fn par2text(p: &Paragraph) -> String {
    match p {
        Paragraph::Sentence { s } => return si2text(s),
        _ => String::new(),
    }
}

fn par2html(p: &Paragraph) -> String {
    match p {
        Paragraph::Sentence { s } => return si2html(s),
        _ => String::new(),
    }
}

// Returns the description for the <head> tag
fn get_description(
    lang: &str,
    a: &ParsedArticleAnalyzed,
    meta: &MetaJson,
) -> Result<String, String> {
    let try1 = a.subtitle.get(0).map(|s| par2html(s)).unwrap_or_default();
    if !try1.trim().is_empty() {
        return Ok(try1.trim().to_string());
    }
    let try1 = a.summary.get(0).map(par2html).unwrap_or_default();
    if !try1.trim().is_empty() {
        return Ok(try1.trim().to_string());
    }
    let sec1 = a
        .sections
        .get(0)
        .and_then(|s| s.pars.get(0))
        .map(par2html)
        .unwrap_or_default();
    if !sec1.trim().is_empty() {
        return Ok(sec1.trim().to_string());
    }

    get_string(meta, lang, "index-desc")
}

fn generate_dropcap_css(a: &ParsedArticleAnalyzed) -> String {
    if a.is_prayer() {
        return String::new();
    }

    let try1 = a.summary.get(0).map(par2text).unwrap_or_default();
    let sec1 = a
        .sections
        .get(0)
        .and_then(|s| s.pars.get(0))
        .map(par2text)
        .unwrap_or_default();
    let mut c = None;
    if !try1.trim().is_empty() {
        c = try1.trim().chars().next()
    } else if !sec1.trim().is_empty() {
        c = sec1.trim().chars().next();
    }

    let c = match c {
        Some(s) => {
            if s.is_ascii_alphabetic() {
                s.to_ascii_uppercase()
            } else {
                return String::new();
            }
        }
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
        format!(
            "    src: url('/static/font/kanzlei/Kanzlei-Initialen-{c}.ttf') format('truetype');"
        ),
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
    let s = strip_comments(s);
    /*/
    use minifier::css;
    let s = match css::minify(&s) {
        Ok(o) => o.to_string(),
        Err(e) => {
            println!("error cssmin: {e:?}");
            let _ = std::fs::write("./output.css", &s);
            s.to_string()
        },
    };
     */
    s
}

fn get_string(meta: &MetaJson, lang: &str, key: &str) -> Result<String, String> {
    Ok(meta
        .strings
        .get(lang)
        .ok_or_else(|| format!("meta.json: strings: unknown lang {lang}"))?
        .get(key)
        .ok_or_else(|| format!("meta.json: strings: {lang}: missing key {key}"))?
        .clone())
}

fn head(
    a: &ParsedArticleAnalyzed,
    lang: &str,
    title_id: &str,
    meta: &MetaJson,
) -> Result<String, String> {
    let darklight = include_str!("../../templates/darklight.html");
    let head_css = include_str!("../../static/css/head2.css").to_string();
    let toc = include_str!("../../static/css/TOC.css");
    let page_toolbar = include_str!("../../static/css/PAGE_TOOLBAR.css");
    let img_css = include_str!("../../static/css/FIGURE.css");
    let floating_header = include_str!("../../static/css/FLOATING_HEADER.css");
    let noscript_style = include_str!("../../static/css/noscript.css");
    let footnotes = if a.footnotes.is_empty() {
        String::new()
    } else {
        include_str!("../../static/css/FOOTNOTE.css").to_string()
    };

    let final_css = head_css + page_toolbar + toc + img_css + floating_header + &footnotes;
    let critical_css = minify_css(&final_css);
    let critical_css_2 = "<style id='critical-css'>".to_string() + &critical_css + "    </style>";

    let title = get_title(lang, a, meta)?;
    let description = get_description(lang, a, meta)?.replace("\"", "'");
    let drc = format!("<style>{}</style>", generate_dropcap_css(a));
    let page_href = get_root_href().to_string() + "/" + lang + "/" + title_id;

    let mut head = include_str!("../../templates/head.html").to_string();
    head = head.replace("<!-- DARKLIGHT_STYLES -->", &darklight);
    head = head.replace("<!-- CRITICAL_CSS -->", &critical_css_2);
    head = head.replace("<!-- DROPCAP_CSS -->", &drc);
    head = head.replace(
        "<!-- NOSCRIPT -->",
        &format!("<style>{}</style>", noscript_style),
    );

    head = head.replace("$$TITLE$$", &title);
    head = head.replace("$$DESCRIPTION$$", &description);
    head = head.replace("$$TITLE_ID$$", title_id);
    head = head.replace("$$KEYWORDS$$", &a.tags.join(", "));
    head = head.replace("$$DATE$$", &a.date);
    head = head.replace("$$AUTHOR$$", &a.authors.join(", "));
    head = head.replace(
        "$$IMG$$",
        &a.img.as_ref().map(|s| s.href.clone()).unwrap_or_default(),
    );
    head = head.replace(
        "$$IMG_ALT$$",
        &a.img.as_ref().map(|s| s.title.clone()).unwrap_or_default(),
    );
    head = head.replace("$$LANG$$", lang);
    head = head.replace("$$ROOT_HREF$$", &get_root_href());
    head = head.replace("$$PAGE_HREF$$", &page_href);
    head = head.replace(
        "$$SKIP_TO_MAIN_CONTENT$$",
        &get_string(meta, lang, "page-smc")?,
    );
    head = head.replace("$$CONTACT_URL$$", &get_string(meta, lang, "link-about")?);
    head = head.replace("$$SLUG$$", title_id);

    Ok(head)
}

fn header_navigation(lang: &str, display_logo: bool, meta: &MetaJson) -> Result<String, String> {
    let homepage_logo = include_str!("../../static/img/logo/logo-smooth-path.svg");
    let logo = if display_logo {
        let homepage_link = get_root_href().to_string() + "/" + lang;
        let hpd = get_string(meta, lang, "nav-homepage-desc")?;
        let logo1 = format!("<a class='logo has-content' rel='home me contents' href='{homepage_link}' data-attribute-title='{hpd}'>");
        let logo2 = format!("<svg class='logo-image' viewBox='0 0 40 75'>{homepage_logo}</svg>");
        vec![logo1, logo2, "</a>".to_string()].join("")
    } else {
        String::new()
    };

    let mut header_nav = include_str!("../../templates/header-navigation.html").to_string();

    header_nav = header_nav.replace("$$HOMEPAGE_LOGO$$", &logo);
    header_nav = header_nav.replace("$$TOOLS_DESC$$", &get_string(meta, lang, "nav-tools-desc")?);
    header_nav = header_nav.replace(
        "$$TOOLS_TITLE$$",
        &get_string(meta, lang, "nav-tools-title")?,
    );
    header_nav = header_nav.replace("$$TOOLS_LINK$$", &get_string(meta, lang, "nav-tools-link")?);
    header_nav = header_nav.replace("$$ABOUT_DESC$$", &get_string(meta, lang, "nav-about-desc")?);
    header_nav = header_nav.replace(
        "$$ABOUT_TITLE$$",
        &get_string(meta, lang, "nav-about-title")?,
    );
    header_nav = header_nav.replace("$$ABOUT_LINK$$", &get_string(meta, lang, "nav-about-link")?);
    header_nav = header_nav.replace(
        "$$ALL_ARTICLES_TITLE$$",
        &get_string(meta, lang, "nav-articles-title")?,
    );
    header_nav = header_nav.replace(
        "$$ALL_ARTICLES_DESC$$",
        &get_string(meta, lang, "nav-articles-desc")?,
    );
    header_nav = header_nav.replace(
        "$$ALL_ARTICLES_LINK$$",
        &get_string(meta, lang, "nav-articles-link")?,
    );
    header_nav = header_nav.replace(
        "$$NEWEST_DESC$$",
        &get_string(meta, lang, "nav-newest-desc")?,
    );
    header_nav = header_nav.replace(
        "$$NEWEST_TITLE$$",
        &get_string(meta, lang, "nav-newest-title")?,
    );
    header_nav = header_nav.replace(
        "$$NEWEST_LINK$$",
        &get_string(meta, lang, "nav-newest-link")?,
    );
    header_nav = header_nav.replace("$$SHOP_DESC$$", &get_string(meta, lang, "nav-shop-desc")?);
    header_nav = header_nav.replace("$$SHOP_TITLE$$", &get_string(meta, lang, "nav-shop-title")?);
    header_nav = header_nav.replace("$$SHOP_LINK$$", &get_string(meta, lang, "nav-shop-link")?);

    Ok(header_nav)
}

fn link_tags(lang: &str, tags: &[String], meta: &MetaJson) -> Result<String, String> {
    let root_href = get_root_href();

    let t_descr_string = get_string(meta, lang, "link-tags-descr")?;
    let t_url = get_string(meta, lang, "nav-articles-link")?;

    let tags_str = tags.iter().map(|t| {
        let t_descr = t_descr_string.replace("$$TAG$$", t);
        let t1 = format!("<a href='{root_href}/{t_url}#{t}'");
        let t2 = "class='link-tag link-page link-annotated icon-not has-annotation spawns-popup' rel='tag' ";
        let t3 = format!(" data-attribute-title='{t_descr}'>{t}</a>");
        t1 + t2 + &t3
    }).collect::<Vec<_>>().join(", ");

    Ok(format!(
        "<div class='link-tags' style='margin: 10px 0px;'><p>{tags_str}</p></div>"
    ))
}

fn gen_section_id(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

fn table_of_contents(
    lang: &str,
    a: &ParsedArticleAnalyzed,
    meta: &MetaJson,
) -> Result<String, String> {
    if a.is_prayer() {
        return Ok(String::new());
    }

    if a.sections.is_empty() {
        return Ok(String::new());
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
            "<a href='#{section_id}' id='toc-{section_id}' class='link-self decorate-not has-content spawns-popup'>{header}</a>"
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

    let collapse_button_title = get_string(meta, lang, "collapse-button-title")?;
    let footnotes_title = get_string(meta, lang, "footnotes-title")?;
    let similar_title = get_string(meta, lang, "similar-title")?;
    let bibliography_title = get_string(meta, lang, "bibliography-title")?;
    let backlinks_title = get_string(meta, lang, "backlinks-title")?;

    let s = "class='link-self decorate-not has-content spawns-popup'";

    if !a.footnotes.is_empty() {
        target += &format!(
            "<li><a {s} id='toc-footnotes' href='#{footnotes_id}'>{footnotes_title}</a></li>"
        );
    }

    if !a.bibliography.is_empty() {
        target += &format!("<li><a {s} id='toc-bibliography' href='#{bibliography_id}'>{bibliography_title}</a></li>");
    }

    if !a.backlinks.is_empty() {
        target += &format!(
            "<li><a {s} id='toc-backlinks' href='#{backlinks_id}'>{backlinks_title}</a></li>"
        );
    }

    if !a.similar.is_empty() {
        target +=
            &format!("<li><a {s} id='toc-similar' href='#{similar_id}'>{similar_title}</a></li>");
    }

    target += &format!("</ul>");
    target += &format!("<button class='toc-collapse-toggle-button' title='{collapse_button_title}' tabindex='-1'><span></span></button>");
    target += &format!("</div>");

    Ok(target)
}

fn page_desciption(
    lang: &str,
    a: &ParsedArticleAnalyzed,
    meta: &MetaJson,
) -> Result<String, String> {
    if a.is_prayer() {
        return Ok(String::new());
    }
    let descr = get_description(lang, a, meta)?;
    Ok(format!("<div class='page-description' style='max-width: 500px;margin: 10px auto;'><p>{descr}</p></div>"))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct Tags {
    ibelievein: Vec<IBelieveIn>,
    iwanttolearn: BTreeMap<Slug, IwantToLearn>,
    tags: BTreeMap<String, String>,
    ressources: Vec<TagSection1>,
    shop: Vec<TagSection2>,
    about: Vec<TagSection3>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MetaJson {
    // translation strings
    owner: Option<String>,
    #[serde(default)]
    strings: BTreeMap<Lang, BTreeMap<String, String>>,
    #[serde(default)]
    authors: BTreeMap<String, Author>,
    #[serde(default)]
    tags: BTreeMap<Lang, Tags>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Author {
    displayname: String,
    #[serde(default)]
    contact: Option<String>,
    #[serde(default)]
    donate: BTreeMap<String, String>,
}

fn read_meta_json(s: &str) -> MetaJson {
    serde_json::from_str(&s).unwrap_or_default()
}

fn page_metadata(lang: &str, a: &ParsedArticleAnalyzed, meta: &MetaJson) -> Result<String, String> {
    if a.is_prayer() {
        return Ok(String::new());
    }

    let mut page_meta = include_str!("../../templates/page-metadata.html").to_string();
    let date = a.date.clone();
    let date_desc = date.clone();
    let date_title = date.clone();

    let authors_link = a.authors.iter().map(|s| {

        let id = s.replace(":", "-");
        let name = meta.authors.get(s).map(|q| &q.displayname)
        .ok_or_else(|| format!("author {s} not found for article {}", a.title))?;

        let u = "/static/img/icon/icons.svg#info-circle-regular";
        let style = format!("data-link-icon='info-circle-regular' data-link-icon-type='svg' style=\"--link-icon-url: url('{u}');\"");
        let classes = "class='backlinks link-self has-icon has-content spawns-popup has-indicator-hook'";

        let mut link = format!("<a href='/{lang}/author/{id}' data-attribute-title='{name}' {style} {classes}>");
        link += &format!("{name}<span class='link-icon-hook'>⁠</span></a>");

        Ok(link)
    }).collect::<Result<Vec<_>, String>>()?.join(", ");

    let backlinks_desc = get_string(meta, lang, "meta-backlinks-desc")?;
    let backlinks_title = get_string(meta, lang, "meta-backlinks-title")?;
    let similar_desc = get_string(meta, lang, "meta-similar-desc")?;
    let similar_title = get_string(meta, lang, "meta-similar-title")?;
    let bibliography_desc = get_string(meta, lang, "meta-bibliography-desc")?;
    let bibliography_title = get_string(meta, lang, "meta-bibliography-title")?;

    page_meta = page_meta.replace("$$DATE_DESC$$", &date_desc);
    page_meta = page_meta.replace("$$DATE_TITLE$$", &date_title);

    if !a.backlinks.is_empty() {
        let mut bl = include_str!("../../templates/page-metadata.backlinks.html").to_string();
        bl = bl.replace("$$BACKLINKS_DESC$$", &backlinks_desc);
        bl = bl.replace("$$BACKLINKS_TITLE$$", &backlinks_title);
        page_meta = page_meta.replace("<!-- BACKLINKS_DOTTED -->", &bl);
    }

    if !a.similar.is_empty() {
        let mut bl = include_str!("../../templates/page-metadata.similar.html").to_string();
        bl = bl.replace("$$SIMILAR_DESC$$", &similar_desc);
        bl = bl.replace("$$SIMILAR_TITLE$$", &similar_title);
        page_meta = page_meta.replace("<!-- SIMILAR_DOTTED -->", &bl);
    }

    if !a.bibliography.is_empty() {
        let mut bl = include_str!("../../templates/page-metadata.bibliography.html").to_string();
        bl = bl.replace("$$BIBLIOGRAPHY_DESC$$", &bibliography_desc);
        bl = bl.replace("$$BIBLIOGRAPHY_TITLE$$", &bibliography_title);
        page_meta = page_meta.replace("<!-- BIBLIOGRAPHY_DOTTED -->", &bl);
    }

    page_meta = page_meta.replace("<!-- AUTHORS -->", &authors_link);

    Ok(page_meta)
}

fn render_paragraph(lang: &str, par: &Paragraph, is_abstract: bool, article_id: &str) -> String {
    let (r, v) = rv();
    let mut target = String::new();
    match par {
        Paragraph::Sentence { s } => {
            if s.is_empty() {
                return String::new();
            }
            target += "<p class='first-graf' style='margin-top:10px;'>";
            for (i, item) in s.iter().enumerate() {
                match item {
                    SentenceItem::Text { text } => {
                        target += &text.replace("[R]: ", &r).replace("[V]: ", &v);
                    }
                    SentenceItem::Link { l } => {
                        target += &format!("<a class='link-annotated link-page spawns-popup' id='{}' href='{}' title='{}'>{}</a>", l.id, l.href, l.title, l.text);
                    }
                    SentenceItem::Footnote { id } => {
                        target += &format!("<a href='$$PAGE_HREF$$#fn{id}' class='footnote-ref spawns-popup' id='fnref{id}' role='doc-noteref'><sup>{id}</sup></a>")
                    }
                }
            }
            target += "</p>";
        }
        Paragraph::Quote { q } => {
            let lv = if is_abstract { 2 } else { 1 };
            target += &format!("<blockquote class='blockquote-level-{lv}' style='margin-top:10px;margin-bottom: 10px;'>");
            if !q.title.is_empty() {
                target += "<strong>";
                target += &q.title;
                target += "</strong>";
            }

            target += &q.quote.iter().map(|p| match p {
                Paragraph::Quote { q } => {
                    let content = q.quote.iter()
                    .map(|p| format!("<p class='first-block first-graf'>{}</p>", par2html(&p))).collect::<Vec<_>>()
                    .join("\r\n");
                    format!("<blockquote class='blockquote-level-{}' style='margin-top:10px;margin-bottom: 10px;'>{content}</blockquote>", lv + 1)
                },
                Paragraph::Sentence { .. } => {
                    format!("<p class='first-block first-graf'>{}</p>", par2html(&p))
                },
                Paragraph::Image { .. } => {
                    String::new()
                },
            }).collect::<Vec<_>>().join("");

            if q.author.is_some() || q.source.is_some() {
                target += "<em style='padding-left:10px;'>";
                if let Some(Link {
                    text,
                    href,
                    title,
                    id,
                }) = q.author.as_ref()
                {
                    target += &format!("<a class='link-annotated link-page spawns-popup' id='{id}' title='{title}' href='{href}'>{text}</a> ");
                }

                if let Some(Link {
                    text,
                    href,
                    title,
                    id,
                }) = q.source.as_ref()
                {
                    if q.author.is_some() {
                        target += "&nbsp;—&nbsp;";
                    }
                    target += &format!("<a class='link-annotated link-page spawns-popup' id='{id}' title='{title}' href='{href}'>{text}</a> ");
                }
                target += "</em>"
            }

            target += "</blockquote>"
        }
        Paragraph::Image { i } => {
            let href = if i.href.contains("://") {
                i.href.clone()
            } else {
                get_root_href().to_string() + "/articles/" + lang + "/" + article_id + "/" + &i.href
            };

            target += &render_image(&Image {
                href: href,
                alt: i.alt.clone(),
                title: i.title.clone(),
                inline: i.inline,
            });
        }
    }

    target
}

fn render_image(i: &Image) -> String {
    // TODO: width="1400" height="1400" data-aspect-ratio="1 / 1" style="aspect-ratio: 1 / 1; width: 678px;"
    let float = include_str!("../../templates/figure.float.html");
    let template = match i.inline.unwrap_or_default() {
        ImageAlignment::FullWidth => include_str!("../../templates/figure.html").to_string(),
        ImageAlignment::Left(px) => float
            .replace("$$MAX_WIDTH$$", &format!("max-width:{px}px;"))
            .replace("$$DIRECTION$$", "left"),
        ImageAlignment::Right(px) => float
            .replace("$$MAX_WIDTH$$", &format!("max-width:{px}px;"))
            .replace("$$DIRECTION$$", "right"),
    };

    template
        .replace("$$IMG_ALT$$", &i.alt)
        .replace("$$IMG_HREF$$", &i.href)
        .replace("$$IMG_CAPTION$$", &i.title)
}

fn body_abstract(lang: &str, article_id: &str, is_prayer: bool, summary: &[Paragraph]) -> String {
    let mut target = String::new();

    if summary.is_empty() {
        return target;
    }

    // body_abstract
    if !is_prayer {
        target += "<blockquote class='blockquote-level-1 block' style='display:flex;flex-direction:column;'>";
        if let Some(first) = summary.get(0).and_then(|q| q.as_sentence()?.get(0)?.text()) {
            let drc = first.chars().next().unwrap_or(' ');
            let rest = first.chars().skip(1).collect::<String>();
            target += "<p class='first-block first-graf intro-graf dropcap-kanzlei' style='--bsm: 0;display:inline;float:left;min-height:7em;'>";
            target += &format!("<span class='dropcap'>{drc}</span>");
            target += &rest;
            target += "</p>";
        }
    }

    for par in summary.iter().skip(if is_prayer { 0 } else { 1 }) {
        target += &render_paragraph(lang, par, true, article_id);
    }

    if !is_prayer {
        target += "</blockquote>";
    }

    target
}

fn render_section(
    lang: &str,
    a: &ArticleSection,
    slug: &str,
    meta: &MetaJson,
) -> Result<String, String> {
    let mut section = include_str!("../../templates/section.html").to_string();

    let first_par = a
        .pars
        .get(0)
        .map(|p| render_paragraph(lang, p, false, slug))
        .unwrap_or_default();
    let other_pars = a
        .pars
        .iter()
        .skip(1)
        .map(|p| render_paragraph(lang, p, false, slug))
        .collect::<Vec<_>>()
        .join("\r\n");

    let header = &a.title;
    let level = a.indent;
    let section_id = gen_section_id(&header);
    let section_descr = get_string(meta, lang, "section-link-to")?.replace("$$HEADER$$", &header);

    section = section.replace("$$LEVEL$$", &level.saturating_sub(1).to_string());
    section = section.replace("$$SECTION_ID$$", &section_id);
    section = section.replace("$$SECTION_DESCR$$", &section_descr);
    section = section.replace("$$SECTION_TITLE$$", &header);
    section = section.replace("<!-- FIRST_PARAGRAPH -->", &first_par);

    section += &other_pars;

    Ok(section)
}

fn body_content(
    lang: &str,
    slug: &str,
    sections: &[ArticleSection],
    meta: &MetaJson,
) -> Result<String, String> {
    Ok(sections
        .iter()
        .map(|q| render_section(lang, q, slug, meta))
        .collect::<Result<Vec<_>, _>>()?
        .join("\r\n"))
}

fn body_noscript() -> String {
    include_str!("../../templates/body-noscript.html").to_string()
}

fn donate(lang: &str, a: &ParsedArticleAnalyzed, meta: &MetaJson) -> Result<String, String> {
    let wc = a
        .sections
        .iter()
        .flat_map(|s| &s.pars)
        .map(|s| s.word_count())
        .sum::<usize>();
    let auth = a
        .authors
        .iter()
        .filter_map(|a| meta.authors.get(a).map(|q| (a, q)))
        .collect::<Vec<_>>();
    let donatable_author = auth.iter().find(|(_, s)| !s.donate.is_empty());

    if auth.is_empty() || a.is_prayer() || wc < 500 || donatable_author.is_none() {
        return Ok(String::new());
    }

    let (_, donatable_author) = donatable_author.unwrap();

    let all_authors = auth
        .iter()
        .map(|(id, a)| {
            format!(
                "<a href='/{lang}/author/{}'>{}</a>",
                id.replace(":", "-"),
                a.displayname
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    let donate_1 = get_string(meta, lang, "donate-1")?.replace("$$AUTHORS$$", &all_authors);

    render_donate_section_internal(lang, &donate_1, &donatable_author, meta)
}
fn render_donate_section_internal(
    lang: &str,
    donate_1: &str,
    donatable_author: &Author,
    meta: &MetaJson,
) -> Result<String, String> {
    let dn_methods = donatable_author
        .donate
        .iter()
        .map(|(id, link)| {
            let id = match id.as_str() {
                "ko-fi" => "Ko-Fi",
                "paypal" => "PayPal",
                "github" => "GitHub Sponsors",
                o => o,
            };
            format!("<a href='{link}'>{}</a>", id)
        })
        .collect::<Vec<_>>()
        .join(" / ");

    let donate_2 = get_string(meta, lang, "donate-2")?.replace("$$DONATION_METHODS$$", &dn_methods);

    let donate_svg = match lang {
        "de" => include_str!("../../static/img/donate/de.svg").to_string(),
        "en" => include_str!("../../static/img/donate/en.svg").to_string(),
        "br" => include_str!("../../static/img/donate/br.svg").to_string(),
        "fr" => include_str!("../../static/img/donate/fr.svg").to_string(),
        "es" => include_str!("../../static/img/donate/es.svg").to_string(),
        _ => String::new(),
    }
    .replace("<svg ", "<svg style='max-height:50px;' ");
    let mut donate = include_str!("../../templates/donate.html").to_string();
    donate = donate.replace("$$DONATE_SVG$$", &format!("/static/img/donate/{lang}.svg"));
    donate = donate.replace(
        "$$DONATE_TEXT$$",
        &(donate_1.to_string() + "&nbsp;" + &donate_2),
    );
    donate = donate.replace("<!-- DONATE_SVG -->", &donate_svg);
    Ok(donate)
}

fn site_author_donation(lang: &str, meta: &MetaJson) -> Result<String, String> {
    let author_id = meta
        .owner
        .as_deref()
        .ok_or_else(|| format!("missing site owner in meta.json"))?;

    let dn_author = meta
        .authors
        .get(author_id)
        .ok_or_else(|| format!("missing site author {author_id}"))?;

    let all_authors = format!(
        "<a href='/{lang}/author/{}'>{}</a>",
        author_id.replace(":", "-"),
        dn_author.displayname
    );

    let donate_1 = get_string(meta, lang, "donate-3")?.replace("$$AUTHORS$$", &all_authors);

    render_donate_section_internal(lang, &donate_1, dn_author, meta)
}

fn footnotes(lang: &str, a: &ParsedArticleAnalyzed, meta: &MetaJson) -> Result<String, String> {
    if a.footnotes.is_empty() {
        return Ok(String::new());
    }

    let q = get_string(meta, lang, "footnotes-title")?;
    let content = a
        .footnotes
        .iter()
        .map(|q| {
            include_str!("../../templates/footnote.html")
                .replace("$$FOOTNOTE_HTML_BACKLINK$$", &format!("fnref{}", q.id))
                .replace("$$FOOTNOTE_TITLE$$", &q.id)
                .replace("$$FOOTNOTE_HTML_ID$$", &format!("fn{}", q.id))
                .replace("$$FOOTNOTE_CONTENT$$", &&si2html(&q.text))
        })
        .collect::<Vec<_>>()
        .join("\r\n");

    let mut s = include_str!("../../templates/footnotes.html").to_string();
    s = s.replace("$$FOOTNOTES_TITLE$$", &q);
    s = s.replace("<!-- FOOTNOTES -->", &content);
    Ok(s)
}

fn backlinks(lang: &str, a: &ParsedArticleAnalyzed, meta: &MetaJson) -> Result<String, String> {
    if a.backlinks.is_empty() {
        return Ok(String::new());
    }
    let s = get_string(meta, lang, "backlinks-title")?;
    Ok(render_index_section(
        lang,
        "backlinks",
        "",
        &s,
        &a.backlinks,
        false,
    ))
}

fn similars(lang: &str, a: &ParsedArticleAnalyzed, meta: &MetaJson) -> Result<String, String> {
    if a.similar.is_empty() {
        return Ok(String::new());
    }
    let s = get_string(meta, lang, "similar-title")?;
    Ok(render_index_section(
        lang, "similar", "", &s, &a.similar, true,
    ))
}

fn bibliography(lang: &str, a: &ParsedArticleAnalyzed, meta: &MetaJson) -> Result<String, String> {
    if a.bibliography.is_empty() {
        return Ok(String::new());
    }

    let bib = a
        .bibliography
        .iter()
        .map(|l| SectionLink {
            id: Some(l.id.clone()),
            slug: l.href.clone(),
            title: if l.title.is_empty() {
                l.text.clone()
            } else {
                l.title.clone()
            },
        })
        .collect::<Vec<_>>();

    let s = get_string(meta, lang, "bibliography-title")?;

    Ok(render_index_section(
        lang,
        "bibliography",
        "",
        &s,
        &bib,
        true,
    ))
}

fn body_footer(lang: &str, a: &ParsedArticleAnalyzed, meta: &MetaJson) -> Result<String, String> {
    let home = get_string(meta, lang, "nav-homepage-desc")?;
    let top = get_string(meta, lang, "go-to-top")?;
    let search = get_string(meta, lang, "searchpage-title")?;

    let mut footer = include_str!("../../templates/footer.html").to_string();
    footer = footer.replace("$$HOME$$", &home);
    footer = footer.replace("$$TOP$$", &top);
    footer = footer.replace("$$SEARCH$$", &search);

    Ok(footer.to_string())
}

fn rosary_template(lang: &str) -> rosary::RosaryTemplates {
    match lang {
        "de" => rosary::RosaryTemplates {
            main_html: include_str!("../../templates/tools.rosary.de.html").to_string(),
            outro_html: include_str!("../../templates/tools.rosary.outro.de.html").to_string(),
            ourfather_html: include_str!("../../templates/tools.rosary.ourfather.de.html")
                .to_string(),
            glorybe_html: include_str!("../../templates/tools.rosary.glorybe.de.html").to_string(),
            fatima_html: include_str!("../../templates/tools.rosary.fatima.de.html").to_string(),
            nav_html: include_str!("../../templates/tools.rosary.nav.de.html").to_string(),
            mystery_section_html: include_str!("../../templates/tools.rosary.mystery.html")
                .to_string(),
        },
        "en" => rosary::RosaryTemplates {
            main_html: include_str!("../../templates/tools.rosary.en.html").to_string(),
            outro_html: include_str!("../../templates/tools.rosary.outro.en.html").to_string(),
            ourfather_html: include_str!("../../templates/tools.rosary.ourfather.en.html")
                .to_string(),
            glorybe_html: include_str!("../../templates/tools.rosary.glorybe.en.html").to_string(),
            fatima_html: include_str!("../../templates/tools.rosary.fatima.en.html").to_string(),
            nav_html: include_str!("../../templates/tools.rosary.nav.en.html").to_string(),
            mystery_section_html: include_str!("../../templates/tools.rosary.mystery.html")
                .to_string(),
        },
        _ => RosaryTemplates::default(),
    }
}

fn rosary_mysteries() -> rosary::RosaryMysteries {
    match serde_json::from_str(include_str!("../../mysteries.json")) {
        Ok(o) => o,
        Err(e) => {
            println!("ERROR parsing mysteries.json: {}", e);
            RosaryMysteries::default()
        }
    }
}

fn article2html(
    lang: &str,
    slug: &str,
    a: &ParsedArticleAnalyzed,
    articles_by_tag: &mut ArticlesByTag,
    articles_by_date: &mut ArticlesByDate,
    meta: &MetaJson,
) -> Result<String, String> {
    static HTML: &str = include_str!("../../templates/lorem.html");

    if a.tags.is_empty() {
        println!("article {lang}/{slug} has no tags");
    }

    for t in a.tags.iter() {
        articles_by_tag
            .entry(lang.to_string())
            .or_insert_with(|| BTreeMap::new())
            .entry(t.to_string())
            .or_insert_with(|| Vec::new())
            .push(SectionLink {
                slug: slug.to_string(),
                title: a.title.to_string(),
                id: None,
            });
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
                    .push(SectionLink {
                        slug: slug.to_string(),
                        title: a.title.to_string(),
                        id: None,
                    });
            }
            None => {
                println!("article {lang}/{slug} has no date");
            }
        };
    }

    let title_id = lang.to_string() + "-" + slug;
    let logo_svg = include_str!("../../static/img/logo/full.svg")
        .replace("<svg ", "<svg style='max-height:50px;' ");

    let mut a = a.clone();

    let content = match (lang, slug) {
        ("de", "rosenkranz") => {
            rosary::generate_rosary(lang, &rosary_template(lang), &rosary_mysteries(), &meta)
        }
        ("en", "rosary") => {
            rosary::generate_rosary(lang, &rosary_template(lang), &rosary_mysteries(), &meta)
        }
        ("en", "online-latin-trainer") => {
            let l = langtrain::TrainLang::Latin;
            let grammar_lessons = l.get_grammar_lessons(lang);
            a.sections.push(ArticleSection {
                title: format!("V01: 1000 words"),
                indent: 2,
                pars: Vec::new(),
            });
            for gl in grammar_lessons.sections.iter() {
                a.sections.push(ArticleSection {
                    title: gl.title.clone(),
                    indent: 2,
                    pars: Vec::new(),
                });
            }
            langtrain::generate_langtrain_content(lang, l, &meta)?
        }
        _ => body_content(lang, &slug, &a.sections, meta)?,
    };

    let a = &a;
    let html = HTML.replace(
        "<!-- HEAD_TEMPLATE_HTML -->",
        &head(a, lang, title_id.as_str(), meta)?,
    );
    let html = html.replace(
        "<!-- HEADER_NAVIGATION -->",
        &header_navigation(lang, true, meta)?,
    );
    let html = html.replace("<!-- LINK_TAGS -->", &link_tags(lang, &a.tags, meta)?);
    let html = html.replace("<!-- TOC -->", &table_of_contents(lang, &a, meta)?);
    let html = html.replace(
        "<!-- PAGE_DESCRIPTION -->",
        &page_desciption(lang, &a, meta)?,
    );
    let html = html.replace("<!-- PAGE_METADATA -->", &page_metadata(lang, &a, meta)?);
    let html = html.replace(
        "<!-- BODY_ABSTRACT -->",
        &body_abstract(lang, slug, a.is_prayer(), &a.summary),
    );
    let html = html.replace("<!-- BODY_CONTENT -->", &content);
    let html = html.replace("<!-- DONATE -->", &donate(lang, &a, meta)?);
    let html = html.replace("<!-- BODY_NOSCRIPT -->", &body_noscript());
    let html = html.replace("<!-- FOOTNOTES -->", &footnotes(lang, a, meta)?);
    let html = html.replace("<!-- BACKLINKS -->", &backlinks(lang, a, meta)?);
    let html = html.replace("<!-- SIMILARS -->", &similars(lang, a, meta)?);
    let html = html.replace("<!-- BIBLIOGRAPHY -->", &bibliography(lang, a, meta)?);
    let html = html.replace("<!-- SVG_LOGO_INLINE -->", &logo_svg);
    let html = html.replace("<!-- BODY_FOOTER -->", &body_footer(lang, a, meta)?);

    let skip = get_string(meta, lang, "page-smc")?;
    let html = html.replace("$$SKIP_TO_MAIN_CONTENT$$", &skip);
    let contact = get_string(meta, lang, "link-about")?;
    let root_href = get_root_href();

    let html = html.replace("$$CONTACT_URL$$", &contact);
    let html = html.replace("$$TITLE$$", &a.title);
    let html = html.replace("$$TITLE_ID$$", &title_id);
    let html = html.replace("$$LANG$$", &lang);
    let html = html.replace("$$SLUG$$", slug);
    let html = html.replace("$$ROOT_HREF$$", &root_href);
    let html = html.replace(
        "$$PAGE_HREF$$",
        &(root_href.to_string() + "/" + lang + "/" + slug),
    );

    Ok(html)
}

fn render_page_author_pages(
    articles: &AnalyzedArticles,
    meta: &MetaJson,
) -> Result<BTreeMap<String, Vec<(String, String)>>, String> {
    let mut finalmap = BTreeMap::new();
    for lang in articles.map.keys() {
        let contact_str = get_string(meta, lang, "author-contact")?;
        let donate_str = get_string(meta, lang, "author-donate")?;

        for (id, v) in meta.authors.iter() {
            let name = &v.displayname;
            let contact_url = v.contact.as_deref();
            let mut dn = String::new();
            for (platform, link) in v.donate.iter() {
                let s = match platform.as_str() {
                    "paypal" => format!("<p><a href='{link}'>PayPal</a></p>"),
                    "github" => format!("<p><a href='{link}'>GitHub Sponsors</a></p>"),
                    "ko-fi" => format!("<p><a href='{link}'>Ko-Fi</a></p>"),
                    _ => {
                        return Err(format!(
                            "unknown platform {platform} for user {id} in authors.json"
                        ))
                    }
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

            finalmap
                .entry(lang.clone())
                .or_insert_with(|| Vec::new())
                .push((id.to_lowercase().replace(":", "-"), t));
        }
    }

    Ok(finalmap)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct SearchIndex {
    git: String,
    articles: BTreeMap<Slug, SearchIndexArticle>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct SearchIndexArticle {
    title: String,
    sha256: String,
}

fn generate_search_index(articles: &AnalyzedArticles, meta: &MetaJson) -> BTreeMap<Lang, SearchIndex> {
    let def = BTreeMap::new();
    meta
        .strings
        .keys()
        .map(|lang| {
            let a = articles.map.get(lang).unwrap_or(&def);
            let s = a
                .values()
                .map(|r| r.sha256.clone())
                .collect::<Vec<_>>()
                .join(" ");
            let version = sha256(&s);
            let articles = a
                .iter()
                .map(|(slug, readme)| {
                    let sia = SearchIndexArticle {
                        title: readme.title.clone(),
                        sha256: readme.sha256.clone(),
                    };
                    (slug.clone(), sia)
                })
                .collect();

            (
                lang.clone(),
                SearchIndex {
                    git: version,
                    articles,
                },
            )
        })
        .collect()
}

type SearchHtmlResult = BTreeMap<Lang, (String, String, String)>;

// Lang => (SearchBarHtml, SearchJS)
fn search_html(articles: &AnalyzedArticles, meta: &MetaJson) -> Result<SearchHtmlResult, String> {
    let def = BTreeMap::new();
    meta.strings.keys()
        .map(|lang| {
            let a = articles.map.get(lang).unwrap_or(&def);
            let s = a
                .values()
                .map(|r| r.sha256.clone())
                .collect::<Vec<_>>()
                .join(" ");
            let version = sha256(&s);

            let searchbar_placeholder = get_string(meta, lang, "searchbar-placeholder")?;
            let searchbar = get_string(meta, lang, "searchbar-text")?;
            let no_results = get_string(meta, lang, "search-no-results")?;
            let searchpage_title = get_string(meta, lang, "searchpage-title")?;
            let searchpage_desc = get_string(meta, lang, "searchpage-desc")?;

            let mut searchbar_html = include_str!("../../templates/searchbar.html").to_string();
            searchbar_html = searchbar_html.replace("$$VERSION$$", &version);
            searchbar_html =
                searchbar_html.replace("$$SEARCHBAR_PLACEHOLDER$$", &searchbar_placeholder);
            searchbar_html = searchbar_html.replace("$$SEARCH$$", &searchbar);

            let mut search_html = include_str!("../../templates/search.html").to_string();
            search_html = search_html.replace("<!-- SEARCH -->", &searchbar_html);
            search_html = search_html.replace(
                "<!-- HEADER_NAVIGATION -->",
                &header_navigation(lang, true, meta)?,
            );
            search_html = search_html.replace("$$LANG$$", lang);
            search_html = search_html.replace("$$ROOT_HREF$$", &get_root_href());

            let parsed = ParsedArticleAnalyzed {
                title: searchpage_title.to_string() + " - dubia.cc",
                summary: vec![Paragraph::Sentence {
                    s: vec![SentenceItem::Text {
                        text: searchpage_desc.to_string(),
                    }],
                }],
                ..Default::default()
            };
            search_html = search_html.replace(
                "<!-- HEAD_TEMPLATE_HTML -->",
                &head(&parsed, lang, &format!("{lang}-search"), meta)?,
            );
            search_html = search_html.replace("$$TITLE$$", &searchpage_title);

            let mut search_js = include_str!("../../static/js/search.js").to_string();
            search_js = search_js.replace("$$LANG$$", lang);
            search_js = search_js.replace("$$VERSION$$", &version);
            search_js = search_js.replace("$$NO_RESULTS$$", &no_results);

            Ok((lang.clone(), (searchbar_html, search_html, search_js)))
        })
        .collect()
}

struct SpecialPage {
    id: String,
    filepath: String,
    title: String,
    description: String,
    content: String,
    special_content: String,
}

fn get_special_pages(
    lang: &str,
    meta: &MetaJson,
    by_tag: &ArticlesByTag,
    by_date: &ArticlesByDate,
) -> Result<Vec<SpecialPage>, String> {
    let tags = meta
        .tags
        .get(lang)
        .ok_or_else(|| format!("unknown language {lang} not found in tags.json"))?;

    let default = BTreeMap::new();
    let default2 = BTreeMap::new();

    let topics_content = render_index_sections(
        lang,
        by_tag
            .get(lang)
            .unwrap_or(&default)
            .iter()
            .filter_map(|(k, v)| {
                let id = k.clone();
                let title = tags.tags.get(&id)?;
                Some(((id.to_string(), title.to_string()), v.clone()))
            })
            .collect(),
    );

    let newest_content = render_index_sections(
        lang,
        by_date
            .get(lang)
            .unwrap_or(&default2)
            .iter()
            .rev()
            .map(|(year, months)| {
                (
                    (format!("y{year}"), year.clone()),
                    months
                        .iter()
                        .flat_map(|(m, days)| {
                            days.iter().flat_map(move |(d, a)| {
                                a.iter().map(move |a| SectionLink {
                                    slug: a.slug.to_string(),
                                    title: format!("{m}-{d}: {}", a.title),
                                    id: None,
                                })
                            })
                        })
                        .collect(),
                )
            })
            .collect(),
    );

    let topics_title = get_string(meta, lang, "special-topics-title")?;
    let topics_html = get_string(meta, lang, "special-topics-path")?;
    let topics_id = get_string(meta, lang, "special-topics-id")?;
    let topics_desc = get_string(meta, lang, "special-topics-desc")?;

    let newest_title = get_string(meta, lang, "special-newest-title")?;
    let newest_html = get_string(meta, lang, "special-newest-path")?;
    let newest_id = get_string(meta, lang, "special-newest-id")?;
    let newest_desc = get_string(meta, lang, "special-newest-desc")?;

    let tools_title = get_string(meta, lang, "special-tools-title")?;
    let tools_html = get_string(meta, lang, "special-tools-path")?;
    let tools_id = get_string(meta, lang, "special-tools-id")?;
    let tools_desc = get_string(meta, lang, "special-tools-desc")?;

    let shop_title = get_string(meta, lang, "special-shop-title")?;
    let shop_html = get_string(meta, lang, "special-shop-path")?;
    let shop_id = get_string(meta, lang, "special-shop-id")?;
    let shop_desc = get_string(meta, lang, "special-shop-desc")?;

    let about_title = get_string(meta, lang, "special-about-title")?;
    let about_html = get_string(meta, lang, "special-about-path")?;
    let about_id = get_string(meta, lang, "special-about-id")?;
    let about_desc = get_string(meta, lang, "special-about-desc")?;

    Ok(vec![
        SpecialPage {
            title: topics_title,
            filepath: topics_html,
            id: topics_id,
            description: topics_desc,
            content: topics_content,
            special_content: String::new(),
        },
        SpecialPage {
            title: newest_title,
            filepath: newest_html,
            id: newest_id,
            description: newest_desc,
            content: newest_content,
            special_content: String::new(),
        },
        SpecialPage {
            title: tools_title,
            filepath: tools_html,
            id: tools_id,
            description: tools_desc,
            content: render_resources_sections(lang, &tags.ressources),
            special_content: String::new(),
        },
        SpecialPage {
            title: shop_title,
            filepath: shop_html,
            id: shop_id,
            description: shop_desc,
            content: render_shop_sections(lang, &tags.shop, meta),
            special_content: site_author_donation(lang, meta).unwrap_or_default(),
        },
        SpecialPage {
            title: about_title,
            filepath: about_html,
            id: about_id,
            description: about_desc,
            content: render_about_sections(&tags.about),
            special_content: String::new(),
        },
    ])
}

fn special2html(
    lang: &str,
    page: &SpecialPage,
    meta: &MetaJson,
) -> Result<(String, String), String> {
    let mut special = include_str!("../../templates/special.html").to_string();
    let a = ParsedArticleAnalyzed {
        title: page.title.to_string(),
        summary: vec![Paragraph::Sentence {
            s: vec![SentenceItem::Text {
                text: page.description.to_string(),
            }],
        }],
        ..Default::default()
    };
    special = special.replace(
        "<!-- HEAD_TEMPLATE_HTML -->",
        &head(&a, lang, &page.id, meta)?,
    );
    special = special.replace("<!-- BODY_NOSCRIPT -->", &page.special_content);
    special = special.replace("<!-- BODY_ABSTRACT -->", &page.content);
    special = special.replace(
        "<!-- HEADER_NAVIGATION -->",
        &header_navigation(lang, true, meta)?,
    );
    special = special.replace("$$TITLE$$", &page.title);
    special = special.replace("$$LANG$$", lang);
    special = special.replace("$$ROOT_HREF$$", &get_root_href());
    special = special.replace(
        "$$PAGE_HREF$$",
        &(get_root_href().to_string() + "/" + lang + "/" + &page.filepath.replace(".html", "")),
    );
    Ok((page.filepath.to_string(), special))
}

fn render_section_items_texts(texts: &[String]) -> String {
    texts
        .iter()
        .map(|s| {
            if s.trim().is_empty() {
                "<br/>".to_string()
            } else if !s.trim().starts_with("<") {
                format!("<p style='text-indent: 0px;'>{s}</p>")
            } else {
                s.clone()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn render_section_items_img(link: &str, img: &str, title: &str) -> String {
    let s1 =
        "justify-content: flex-end;margin-top:10px;width: 100%;min-height: 440px;display: flex;";
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

        let id = match l.id.as_deref() {
            Some(s) => s.to_string(),
            None => slug
                .replace(":", "")
                .replace("/", "")
                .replace(".", "")
                .to_string(),
        };

        vec![
            format!("<li class='block link-modified-recently-list-item dark-mode-invert' style='--bsm:{bsm};'>"),
            format!("  <p class='in-list first-graf block' style='--bsm: 0;'><a href='{final_link}' id='{lang}-{id}' "),
            format!("      class='link-annotated link-page link-modified-recently in-list spawns-popup'"),
            format!("      data-attribute-title='{section_title}'>{section_title}</a></p>"),
            format!("</li>"),
        ].join("\r\n")
    }).collect::<Vec<_>>().join("\r\n")
}

fn render_index_section(
    lang: &str,
    id: &str,
    classes: &str,
    title: &str,
    links: &[SectionLink],
    two_column: bool,
) -> String {
    let mut section_html = include_str!("../../templates/index.section.html").to_string();
    section_html = section_html.replace("$$SECTION_ID$$", id);
    section_html = section_html.replace("$$SECTION_CLASSES$$", classes);
    section_html = section_html.replace("$$SECTION_NAME$$", title);
    section_html = section_html.replace("$$SECTION_NAME_TITLE$$", title);

    let section_items = if two_column {
        let col1 = links
            .iter()
            .enumerate()
            .filter_map(|(i, l)| if i % 2 == 0 { Some(l.clone()) } else { None })
            .collect::<Vec<_>>();
        let col1 = render_section_items(lang, &col1);
        let col2 = links
            .iter()
            .enumerate()
            .filter_map(|(i, l)| if i % 2 != 0 { Some(l.clone()) } else { None })
            .collect::<Vec<_>>();
        let col2 = render_section_items(lang, &col2);
        let cont = format!("<div class='col'>{col1}</div><div class='col'>{col2}</div>");
        format!("<div class='index-section-grid-container'>{cont}</div>")
    } else {
        render_section_items(lang, links)
    };

    section_html = section_html.replace("<!-- SECTION_ITEMS -->", &section_items);
    section_html
}

fn render_index_section_texts(id: &str, classes: &str, title: &str, txts: &[String]) -> String {
    let mut section_html = include_str!("../../templates/index.section.html").to_string();
    section_html = section_html.replace("$$SECTION_ID$$", id);
    section_html = section_html.replace("$$SECTION_CLASSES$$", classes);
    section_html = section_html.replace("$$SECTION_NAME$$", title);
    section_html = section_html.replace("$$SECTION_NAME_TITLE$$", title);
    section_html =
        section_html.replace("<!-- SECTION_ITEMS -->", &&render_section_items_texts(txts));
    section_html
}

fn render_index_section_img(
    lang: &str,
    id: &str,
    title: &str,
    link: &str,
    img: &str,
    t: &str,
    meta: &MetaJson,
) -> String {
    let mut section_html = include_str!("../../templates/index.section.html").to_string();
    section_html = section_html.replace("$$SECTION_ID$$", id);
    let nav_shop_link = get_string(meta, lang, "nav-shop-link").unwrap_or_default();
    section_html = section_html.replace("$$LANG$$", &nav_shop_link);
    section_html =
        section_html.replace("$$PAGE_HREF$$", &(get_root_href().to_string() + "/" + lang));
    section_html = section_html.replace("$$SECTION_CLASSES$$", "");
    section_html = section_html.replace("$$SECTION_NAME$$", title);
    section_html = section_html.replace("$$SECTION_NAME_TITLE$$", title);
    section_html = section_html.replace(
        "<!-- SECTION_ITEMS -->",
        &&render_section_items_img(link, img, t),
    );
    section_html
}

fn render_index_sections(lang: &str, s: Vec<((String, String), Vec<SectionLink>)>) -> String {
    s.iter()
        .map(|((id, title), links)| render_index_section(lang, id, "", title, links, false))
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn render_resources_sections(lang: &str, s: &Vec<TagSection1>) -> String {
    s.iter()
        .map(|s| {
            let section_id = &s.id;
            let section_title = &s.title;
            render_index_section(lang, section_id, "", section_title, &s.links, false)
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn render_shop_sections(lang: &str, s: &Vec<TagSection2>, meta: &MetaJson) -> String {
    s.iter()
        .map(|s| {
            render_index_section_img(
                lang,
                &s.id,
                &s.title,
                &s.link.slug,
                &s.img,
                &s.link.title,
                meta,
            )
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn render_about_sections(s: &Vec<TagSection3>) -> String {
    s.iter()
        .map(|s| render_index_section_texts(&s.id, "", &s.title, &s.texts))
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn render_index_first_section(
    lang: &str,
    tags: &Tags,
    articles: &AnalyzedArticles,
    meta: &MetaJson,
) -> Result<String, String> {
    let mut first_section = include_str!("../../templates/index.first-section.html").to_string();

    let dropdown_svg = r#"
    <svg style='pointer-events: none;position: absolute;top: 15px;left: 10px;'version="1.1" xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="4319.7 0 448 512" preserveAspectRatio="xMinYMin" >
        <g transform="translate(4319.7, 0)"><path d="M441.9 250.1l-19.8-19.8c-4.7-4.7-12.3-4.7-17 0L250 385.4V44c0-6.6-5.4-12-12-12h-28c-6.6 0-12 5.4-12 12v341.4L42.9 230.3c-4.7-4.7-12.3-4.7-17 0L6.1 250.1c-4.7 4.7-4.7 12.3 0 17l209.4 209.4c4.7 4.7 12.3 4.7 17 0l209.4-209.4c4.7-4.7 4.7-12.3 0-17z"></path></g>
    </svg>
    "#;

    let mut options = String::new();
    let mut fs = String::new();
    let mut os = String::new();

    for (i, t) in tags.ibelievein.iter().enumerate() {
        options += &format!("<option value='{}'>{}</option>", t.tag, t.option);

        let featured = t
            .featured
            .iter()
            .filter_map(|id| {
                Some(SectionLink {
                    title: articles.map.get(lang)?.get(id)?.title.clone(),
                    slug: id.to_string(),
                    id: None,
                })
            })
            .collect::<Vec<_>>();

        let classes = "list list-level-1";
        let s = render_index_section(lang, &t.tag, &classes, &t.title, &featured, true);

        if i == 0 {
            fs = s;
        } else {
            os += &s;
        }
    }

    let text_ibelieve = get_string(meta, lang, "i-believe-in")?;

    let base = &base64::encode(serde_json::to_string(&tags.ibelievein).unwrap_or_default());

    first_section = first_section.replace("$$I_BELIEVE_IN$$", &text_ibelieve);
    first_section = first_section.replace("$$ARTICLES$$", &base);
    first_section = first_section.replace("<!-- INITIAL_FIRST_SECTION -->", &fs);
    first_section = first_section.replace("<!-- INITIAL_OTHER_SECTIONS -->", &os);
    first_section = first_section.replace("<!-- OPTIONS -->", &options);
    first_section = first_section.replace("<!-- SELECT_SVG -->", &dropdown_svg);

    Ok(first_section)
}

fn render_other_index_sections(
    lang: &str,
    tags: &Tags,
    articles: &AnalyzedArticles,
) -> Result<String, String> {

    let def = BTreeMap::new();
    let articles = articles
        .map
        .get(lang)
        .unwrap_or(&def);

    let s = tags
        .iwanttolearn
        .iter()
        .map(|(id, v)| {
            let featured = v
                .featured
                .iter()
                .filter_map(|f_id| {
                    let featured_title = articles.get(f_id)?.title.clone();
                    Some(SectionLink {
                        slug: f_id.to_string(),
                        title: featured_title,
                        id: None,
                    })
                })
                .collect::<Vec<_>>();

            render_index_section(lang, id, "", &v.title, &featured, false)
        })
        .collect::<Vec<_>>()
        .join("");

    Ok(format!("<div id='i-want-to-learn-about'>{s}</div>"))
}

fn render_index_html(
    lang: &str,
    articles: &AnalyzedArticles,
    meta: &MetaJson,
    search_html: &SearchHtmlResult,
) -> Result<String, String> {
    let tags = meta
        .tags
        .get(lang)
        .ok_or_else(|| format!("render_index_html: unknown language {lang}"))?;

    let (searchbar_html, _, _) = search_html
        .get(lang)
        .ok_or_else(|| format!("render_index_html (searchbar_html): unknown language {lang}"))?;

    let multilang = include_str!("../../templates/multilang.tags.html");
    let logo_svg = include_str!("../../static/img/logo/full.svg");

    let title = get_title(lang, &ParsedArticleAnalyzed::default(), meta)?;
    let description = get_description(lang, &ParsedArticleAnalyzed::default(), meta)?;
    let keywords = get_string(meta, lang, "index-keywords")?
        .split(",")
        .map(|q| q.trim().to_string())
        .collect();

    let a = ParsedArticleAnalyzed {
        title: title.clone(),
        summary: vec![Paragraph::Sentence {
            s: vec![SentenceItem::Text {
                text: description.clone(),
            }],
        }],
        tags: keywords,
        ..Default::default()
    };

    let select_faith = get_string(meta, lang, "index-select-faith")?;

    let page_help_content = vec![
        get_string(meta, lang, "index-help-1")?,
        get_string(meta, lang, "index-help-2")?,
        get_string(meta, lang, "index-help-3")?,
        get_string(meta, lang, "index-help-4")?,
        get_string(meta, lang, "index-help-5")?,
        get_string(meta, lang, "index-help-6")?,
        get_string(meta, lang, "index-help-7")?,
    ]
    .join("");

    let ims = r#"
    <svg style='position: relative;top: 4px;margin-left:1px;' version="1.1" xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="37778.048 -16 544 544" preserveAspectRatio="xMinYMin" >
        <g fill-rule="evenodd" style="paint-order:normal" transform="translate(37794.048, 0)"><path d="M283.211 512c78.962 0 151.079-35.925 198.857-94.792 7.068-8.708-.639-21.43-11.562-19.35-124.203 23.654-238.262-71.576-238.262-196.954 0-72.222 38.662-138.635 101.498-174.394 9.686-5.512 7.25-20.197-3.756-22.23A258.156 258.156 0 0 0 283.211 0c-141.309 0-256 114.511-256 256 0 141.309 114.511 256 256 256z"></path></g>
    </svg>"#.trim().to_string();

    let ibos = r#"
    <svg style='position: relative;top: 5px;margin-left:1px;' version="1.1" xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="15096.95 0 576 448" preserveAspectRatio="xMinYMin" >
        <g fill-rule="evenodd" style="paint-order:normal" transform="translate(15096.95, 0)"><path d="M 542.22,0.05 C 487.42,3.16 378.5,14.48 311.26,55.64 c -4.64,2.84 -7.27,7.89 -7.27,13.17 V 432.68431 c 0,11.55 12.63,18.85 23.28,13.49 69.18,-34.82 169.23,-44.32 218.7,-46.92 16.89,-0.89 30.02,-14.43 30.02,-30.66 V 30.75 C 576,13.04 560.64,-0.99 542.22,0.05 Z M 264.73,55.64 C 197.5,14.48 88.58,3.17 33.78,0.05 15.36,-0.99 0,13.04 0,30.75 V 368.6 c 0,16.24 13.13,29.78 30.02,30.66 49.49,2.6 149.59,12.11 218.77,46.95 10.62,5.35 23.21,-1.94 23.21,-13.46 V 68.63 c 0,-5.29 -2.62,-10.14 -7.27,-12.99 z"></path></g>
    </svg>"#.trim().to_string();

    let imss = r#"
    <svg style='position: relative;top: 5px;margin-left:1px;' version="1.1" xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="35743.5089 0 640 512" preserveAspectRatio="xMinYMin" >
        <g fill-rule="evenodd" style="paint-order:normal" transform="translate(35743.5089, 0)"><path d="M63.1 351.1c0 35.25 28.75 63.1 63.1 63.1h95.1v83.99c0 9.749 11.25 15.45 19.12 9.7l124.9-93.69l39.37-.0117L63.1 146.9L63.1 351.1zM630.8 469.1l-82.76-64.87c16.77-11.47 27.95-30.46 27.95-52.27V63.1c0-35.25-28.75-63.1-63.1-63.1H127.1c-23.51 0-43.97 12.88-55.07 31.86L38.81 5.128C34.41 1.691 29.19 .0332 24.03 .0332c-7.125 0-14.2 3.137-18.92 9.168c-8.187 10.44-6.365 25.53 4.073 33.7l591.1 463.1c10.5 8.202 25.57 6.333 33.7-4.073C643.1 492.4 641.2 477.3 630.8 469.1z"></path></g>
    </svg>"#.trim().to_string();

    let imag = r#"
    <svg style='position: relative;top: 5px;margin-left:1px;' version="1.1" xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="34696.5839 2.875 17.75 17.75" preserveAspectRatio="xMinYMin" >
        <g fill-rule="evenodd" style="paint-order:normal" transform="translate(34693.7089, 0)"><path d="M15.5 14h-.79l-.28-.27A6.471 6.471 0 0 0 16 9.5 6.5 6.5 0 1 0 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z"></path></g>
    </svg>"#.trim().to_string();

    let igear = r#"
    <svg style='position: relative;top: 5px;' version="1.1" xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="29013.3239 0 512 512" preserveAspectRatio="xMinYMin" >
        <g fill-rule="evenodd" style="paint-order:normal" transform="translate(29013.3239, 0)"><path d="M495.9 166.6c3.2 8.7 .5 18.4-6.4 24.6l-43.3 39.4c1.1 8.3 1.7 16.8 1.7 25.4s-.6 17.1-1.7 25.4l43.3 39.4c6.9 6.2 9.6 15.9 6.4 24.6c-4.4 11.9-9.7 23.3-15.8 34.3l-4.7 8.1c-6.6 11-14 21.4-22.1 31.2c-5.9 7.2-15.7 9.6-24.5 6.8l-55.7-17.7c-13.4 10.3-28.2 18.9-44 25.4l-12.5 57.1c-2 9.1-9 16.3-18.2 17.8c-13.8 2.3-28 3.5-42.5 3.5s-28.7-1.2-42.5-3.5c-9.2-1.5-16.2-8.7-18.2-17.8l-12.5-57.1c-15.8-6.5-30.6-15.1-44-25.4L83.1 425.9c-8.8 2.8-18.6 .3-24.5-6.8c-8.1-9.8-15.5-20.2-22.1-31.2l-4.7-8.1c-6.1-11-11.4-22.4-15.8-34.3c-3.2-8.7-.5-18.4 6.4-24.6l43.3-39.4C64.6 273.1 64 264.6 64 256s.6-17.1 1.7-25.4L22.4 191.2c-6.9-6.2-9.6-15.9-6.4-24.6c4.4-11.9 9.7-23.3 15.8-34.3l4.7-8.1c6.6-11 14-21.4 22.1-31.2c5.9-7.2 15.7-9.6 24.5-6.8l55.7 17.7c13.4-10.3 28.2-18.9 44-25.4l12.5-57.1c2-9.1 9-16.3 18.2-17.8C227.3 1.2 241.5 0 256 0s28.7 1.2 42.5 3.5c9.2 1.5 16.2 8.7 18.2 17.8l12.5 57.1c15.8 6.5 30.6 15.1 44 25.4l55.7-17.7c8.8-2.8 18.6-.3 24.5 6.8c8.1 9.8 15.5 20.2 22.1 31.2l4.7 8.1c6.1 11 11.4 22.4 15.8 34.3zM256 336a80 80 0 1 0 0-160 80 80 0 1 0 0 160z"></path></g>
    </svg>"#.trim().to_string();

    let icons = &[
        ("<span class='icon-moon-solid'></span>", ims),
        ("<span class='icon-book-open-solid'></span>", ibos),
        ("<span class='icon-message-slash-solid'></span>", imss),
        ("<span class='icon-magnifying-glass'></span>", imag),
        ("<span class='icon-gear-solid'></span>", igear),
    ];

    let mut page_help = include_str!("../../templates/navigation-help.html")
        .replace("$$PAGE_HELP$$", &page_help_content);

    for (k, v) in icons {
        page_help = page_help.replace(k, v);
    }

    let page_descr = get_string(meta, lang, "index-subtitle")?;
    let page_description =
        include_str!("../../templates/page-description.html").replace("$$DESCR$$", &page_descr);

    let mut index_body_html = include_str!("../../templates/index-body.html").to_string();
    index_body_html = index_body_html.replace(
        "<!-- SECTIONS -->",
        &render_index_first_section(lang, tags, articles, meta)?,
    );
    index_body_html = index_body_html.replace(
        "$$I_WANT_TO_LEARN_MORE_ABOUT$$",
        &get_string(meta, lang, "i-want-to-learn-more")?,
    );
    index_body_html = index_body_html.replace(
        "<!-- SECTION_EXTRA -->",
        &render_other_index_sections(lang, tags, articles)?,
    );
    index_body_html = index_body_html.replace("<!-- SEARCHBAR -->", &searchbar_html);

    let title_id = format!("{lang}-index");
    let mut index_html = include_str!("../../templates/index.html").to_string();
    index_html = index_html.replace("<!-- BODY_ABSTRACT -->", &index_body_html);
    index_html = index_html.replace("<!-- PAGE_DESCRIPTION -->", &page_description);
    index_html = index_html.replace("<!-- SVG_LOGO_INLINE -->", logo_svg);
    index_html = index_html.replace(
        "<!-- HEAD_TEMPLATE_HTML -->",
        &head(&a, lang, &title_id, meta)?,
    );
    index_html = index_html.replace("<!-- PAGE_HELP -->", &page_help);
    index_html = index_html.replace(
        "<!-- HEADER_NAVIGATION -->",
        &header_navigation(lang, false, meta)?,
    );
    index_html = index_html.replace("<!-- MULTILANG_TAGS -->", multilang);
    index_html = index_html.replace("$$SKIP_TO_MAIN_CONTENT$$", "Skip to main content");
    index_html = index_html.replace("$$TITLE$$", &title);
    index_html = index_html.replace("$$DESCRIPTION$$", &description);
    index_html = index_html.replace("$$TITLE_ID$$", &title_id);
    index_html = index_html.replace("$$LANG$$", lang);
    index_html = index_html.replace("$$SLUG$$", "");
    index_html = index_html.replace("$$SELECT_FAITH$$", &select_faith);
    index_html = index_html.replace("$$ROOT_HREF$$", get_root_href());
    index_html = index_html.replace("$$PAGE_HREF$$", &(get_root_href().to_string() + "/" + lang));
    index_html = index_html.replace(
        "<link rel=\"preload\" href=\"/static/img/logo/logo-smooth.svg\" as=\"image\">",
        "",
    );
    index_html = index_html.replace("<link rel=\"preload\" href=\"/static/font/ssfp/ssp/SourceSansPro-BASIC-Regular.subset.woff2\" as=\"font\" type=\"font/woff2\" crossorigin>", "");

    Ok(index_html)
}

pub fn minify(input: &str) -> Vec<u8> {
    let s = include_str!("../../templates/sw-inject.js");
    // let input = input.replace("<!-- INJECT_SW -->", &format!("<script>{s}</script>"));
    let mut minified = vec![];
    html5minify::Minifier::new(&mut minified)
        .minify(&mut input.as_bytes())
        .expect("Failed to minify HTML");
    minified
}

fn main() -> Result<(), String> {
    // Setup
    let mut cwd = std::env::current_dir().map_err(|e| e.to_string())?;

    while !cwd.join("articles").is_dir() {
        cwd = cwd
            .parent()
            .ok_or("cannot find /articles dir in current path")?
            .to_path_buf();
    }

    let meta = std::fs::read_to_string(&cwd.join("meta.json")).map_err(|e| e.to_string())?;
    let meta_map = read_meta_json(&meta);

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
                &mut articles_by_tag,
                &mut articles_by_date,
                &meta_map,
            );

            match s {
                Ok(s) => {
                    let path = cwd.join(lang);
                    let _ = std::fs::create_dir_all(&path);
                    let _ = std::fs::write(path.join(slug.to_string() + ".html"), &minify(&s));
                }
                Err(e) if e.is_empty() => {}
                Err(q) => return Err(q),
            }
        }
    }

    // Write author pages
    let author_pages = render_page_author_pages(&analyzed, &meta_map)?;
    for (lang, authors) in author_pages.iter() {
        let _ = std::fs::create_dir_all(cwd.join(&lang).join("author"));
        for (a, v) in authors {
            let _ = std::fs::write(
                cwd.join(&lang).join("author").join(&format!("{a}.html")),
                &minify(&v),
            );
        }
    }

    // Generate search index
    let si = generate_search_index(&analyzed, &meta_map);
    for (lang, si) in si.iter() {
        let json = serde_json::to_string(&si).unwrap_or_default();
        let _ = std::fs::write(cwd.join(lang).join("index.json"), json);
    }

    // Write special pages
    let langs = meta_map.strings.keys().cloned().collect::<Vec<_>>();
    for l in langs.iter() {
        let sp = get_special_pages(&l, &meta_map, &articles_by_tag, &articles_by_date)?;
        for s in sp.iter() {
            let (mut path, html) = special2html(&l, s, &meta_map)?;
            if !path.ends_with(".html") {
                path = path + ".html";
            }
            let _ = std::fs::write(cwd.join(l).join(path), &&html);
        }
    }

    // Write index + /search pages
    let si = search_html(&analyzed, &meta_map)?;
    for (lang, (_searchbar_html, search_html, search_js)) in si.iter() {
        let _ = std::fs::create_dir_all(cwd.join(lang));
        let _ = std::fs::write(cwd.join(lang).join("search.js"), search_js);
        let _ = std::fs::write(cwd.join(lang).join("search.html"), &minify(&search_html));
        let index_html = render_index_html(lang, &analyzed, &meta_map, &si)?;
        let _ = std::fs::write(cwd.join(&format!("{lang}.html")), &minify(&index_html));
    }

    // Generate map pages
    resistance::generate_resistance_pages(&cwd, &meta_map)?;

    // Write gitignore
    let _ = std::fs::write(cwd.join(".gitignore"), generate_gitignore(&articles, &meta_map));

    // Write serviceworker
    let _ = std::fs::write(
        cwd.join("sw.js"),
        gen_serviceworker_js(&cwd, &analyzed, &meta_map),
    );

    Ok(())
}
