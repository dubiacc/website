use std::path::Path;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use serde_derive::{Serialize, Deserialize};
use take_until::TakeUntilExt;

#[derive(Debug, Default)]
struct LoadedArticles {
    langs: BTreeMap<String, Articles>,
}

#[derive(Debug, Default)]
struct Articles {
    map: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy)]
enum ArticleType {
    Question,
    Tract,
    Prayer,
}

#[derive(Debug)]
struct VectorizedArticle {
    src: String,
    words: Vec<usize>,
    atype: ArticleType,
    parsed: ParsedArticle,
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

impl VectorizedArticle {
    pub fn analyze(
        &self, 
        id: &str,
        articles: &BTreeMap<String, VectorizedArticle>
    ) -> ParsedArticleAnalyzed {
        let similar = get_similar_articles(self, id, articles);
        ParsedArticleAnalyzed {
            title: self.parsed.title.clone(),
            date: self.parsed.date.clone(),
            tags: self.parsed.tags.clone(),
            authors: self.parsed.authors.clone(),
            sha256: self.parsed.sha256.clone(),
            img: self.parsed.img.clone(),
            summary: self.parsed.summary.clone(),
            sections: self.parsed.sections.clone(),
            related: similar,
            article_abstract: self.parsed.article_abstract.clone(),
            footnotes: self.parsed.footnotes.clone(),
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

#[derive(Debug, Default)]
struct VectorizedArticles {
    map: BTreeMap<String, VectorizedArticle>,
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
        .filter(|(i, s)| s.starts_with("# "))
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

impl Articles {
    pub fn vectorize(&self) -> VectorizedArticles {
    
        fn get_words_of_article(s: &str) -> Vec<&str> {
            s.split_whitespace()
            .filter_map(|s| if s.contains("[") || s.contains("]") || s.len() < 3 { None } else { Some(s) })
            .collect()
        }

        let all_words = self.map.values().flat_map(|c| get_words_of_article(c)).collect::<BTreeSet<_>>();
        let all_words_indexed = all_words.iter().enumerate().map(|(i, s)| (*s, i)).collect::<BTreeMap<_, _>>();
        VectorizedArticles {
            map: self.map.iter().map(|(k, v)| {
                let embedding = get_words_of_article(v)
                .into_iter()
                .filter_map(|q| all_words_indexed.get(q).copied()).collect();
                let atype = ArticleType::new(v);

                (k.clone(), VectorizedArticle {
                    src: v.clone(),
                    words: embedding,
                    atype: atype,
                    parsed: parse_article(v),
                })
            }).collect()
        }
    }
}

/// return similar articles based on string distance for article N
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
        .or_insert_with(|| Articles::default())
        .map.insert(id, contents);
    }

    Ok(LoadedArticles { langs })
}

fn main() -> Result<(), String> {

    let mut cwd = std::env::current_dir()
        .map_err(|e| e.to_string())?;
    
    while !cwd.join("articles").is_dir() {
        cwd = cwd.parent().ok_or("cannot find /articles dir in current path")?.to_path_buf();
    }

    let dir = cwd.join("articles");

    let articles = load_articles(&dir)?.langs;
    let articles = articles.iter().map(|(lang, a)| {
        let vectorized = a.vectorize();
        let s = vectorized.map
            .iter()
            .map(|(k, v)| (k.clone(), v.analyze(k, &vectorized.map)))
            .collect::<BTreeMap<_, _>>();
        (lang.clone(), s)
    }).collect::<BTreeMap<_, _>>();

    let json = serde_json::to_string_pretty(&articles).unwrap_or_default();

    println!("{json}");

    Ok(())
}
