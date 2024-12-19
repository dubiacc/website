use std::path::Path;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use serde_derive::{Serialize, Deserialize};
use take_until::TakeUntilExt;
use split_iter::Splittable;

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
            footnotes: self.parsed.footnotes.clone(),
            bibliography: BTreeMap::new(), // todo
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
    sections: Vec<ArticleSection>,
    related: Vec<String>,
    footnotes: Vec<String>, // BTreeMap<String, Paragraph>,
    bibliography: BTreeMap<String, Paragraph>,
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
    Sentences { s: Vec<Sentence> },
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
        text: String,
        href: String,
        ltype: LinkType,
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
        Paragraph::Sentences {
            s: s.split_inclusive(".").map(|q| Sentence::new(q.trim())).collect(),
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

fn parse_article(s: &str) -> ParsedArticle {
        
    let title = s.lines()
        .filter(|s| s.starts_with("# "))
        .map(|q| q.replace("# ", "").trim().to_string())
        .next()
        .unwrap_or_default();

    let sha256 = sha256(&s);

    let mut codeblock = Vec::new();
    let mut in_cb = false;
    for l in s.lines() {
        if l.contains("```") {
            if in_cb {
                in_cb = false; 
            } else {
                in_cb = codeblock.is_empty();
            }
        } else if in_cb {
            codeblock.push(l.trim().clone());
        }
    }

    let config = serde_json::from_str::<Config>(
        &codeblock.join("\r\n")
    ).unwrap_or_default();

    let lines_before_heading = s.lines()
        .take_until(|s| s.starts_with("# "))
        .filter(|s| !s.starts_with("# "))
        .collect::<Vec<_>>().join("\r\n");

    let mut lines_after_heading = s.lines().rev()
        .take_until(|s| s.starts_with("# "))
        .filter(|s| !s.starts_with("# "))
        .collect::<Vec<_>>();

    lines_after_heading.reverse();
    let lines_after_heading = lines_after_heading.join("\r\n");

    let mut footnotes = Vec::new();
    let mut sections = Vec::new();
    let mut last_section = Vec::new();

    for l in lines_after_heading.lines() {
        if l.trim().starts_with("[^") && l.contains("]:") {
            footnotes.push(l.to_string());
        } else if l.contains("# ") {
            let indent = l.chars().filter(|c| *c == '#').count();
            let title = l.replace("#", "").trim().to_string();
            sections.push(ArticleSection {
                title,
                indent,
                pars: parse_paragraphs(&last_section.join("\r\n"))
            });
            last_section = Vec::new();
        } else {
            last_section.push(l.trim().to_string());
        }
    }

    ParsedArticle {
        title,
        date: config.date,
        tags: config.tags,
        authors: config.authors,
        sha256: sha256,
        img: None,
        summary: parse_paragraphs(&lines_before_heading),
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

        let lines = s.trim().lines()
        .map(|l| l.trim())
        .filter(|l| l.starts_with(">"))
        .map(|l| l.replacen(">", "", 1).trim().to_string())
        .collect::<Vec<_>>();
        
        let title = lines.iter()
            .find(|s| s.starts_with("**"))
            .map(|l| l.replace("**", ""))
            .unwrap_or_default();

        let author_line = lines.iter().find(|s| s.trim().starts_with("--"))?.replacen("--", "", 1);
        let author_link = author_line.split(":").nth(0)?;
        let source = author_line.split(":").nth(1);

        let lines = lines.iter()
            .filter(|s| !s.starts_with("**"))
            .filter(|s| !s.starts_with("--"))
            .cloned()
            .collect::<Vec<_>>();

        let quote = lines
            .split(|s| s.trim().is_empty())
            .map(|q| q.join(" "))
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<String>>();

        Some(Quote {
            title,
            quote,
            author: String::new(),
            author_link: String::new(),
            source: String::new(),
            source_link: String::new(),
        })
    }
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
        quote: vec!["LineA LineB LineC".to_string(), "LineD LineE".to_string()],
        author: "Test".to_string(),
        author_link: "https://wikipedia.org/Test".to_string(),
        source: "de juiribus".to_string(),
        source_link: "test.pdf".to_string(),
    }))

}

impl Sentence {
    fn new(s: &str) -> Self {

        /*
        SentenceItem {
            Text(String),
            Link {
                text: String,
                href: String,
                ltype: LinkType,
            },
            Footnote(String),
        }
        */

        Self {
            items: vec![SentenceItem::Text { text: s.trim().to_string() }]
        }
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
            SentenceItem::Link { text: "link".to_string(), href: "url".to_string(), ltype: LinkType::Other },
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
