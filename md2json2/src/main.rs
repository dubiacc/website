use std::path::Path;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

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
}

impl ParsedArticle {
    pub fn new(s: &str) -> Self {
        
        let title = s.lines()
            .filter(|s| s.starts_with("# "))
            .map(|q| q.replace("# ", "").trim().to_string())
            .next()
            .unwrap_or_default();

        let sha256 = Self::sha256(&s);

        Self {
            title,
            date: String::new(),
            tags: Vec::new(),
            authors: Vec::new(),
            sha256: sha256,
            img: None,
            summary: Vec::new(),
            sections: Vec::new()
        }
    }

    fn sha256(s: &str) -> String {
        use sha2::{Sha256, Digest};
        use base64::Engine;
        let mut hasher = Sha256::new();
        hasher.update(s.as_bytes());
        let result = hasher.finalize();
        base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(result)
    }
}

#[derive(Debug, Default)]
struct ArticleSection {
    title: String,
    indent: usize,
    pars: Vec<Paragraph>,
}

#[derive(Debug)]
enum Paragraph {
    Sentences { s: Vec<Sentence> },
    Quote { q: Quote },
    Image { i: Image }
}

#[derive(Debug, Default)]
struct Sentence {
    items: Vec<SentenceItem>
}

#[derive(Debug)]
enum SentenceItem {
    Text(String),
    Link {
        text: String,
        href: String,
        ltype: LinkType,
    },
    Footnote(String),
}

#[derive(Debug, Copy, Clone)]
enum LinkType {
    Wikipedia,
    Internal,
    Other,
}

#[derive(Debug, Default)]
struct Quote {
    title: String,
    quote: String,
    author: String,
    author_link: String,
    source: String,
    source_link: String,
}

#[derive(Debug, Default)]
struct Image {
    href: String, 
    inline: String,
}

#[derive(Debug, Default)]
struct VectorizedArticles {
    map: BTreeMap<String, VectorizedArticle>,
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
                    parsed: ParsedArticle::new(v),
                })
            }).collect()
        }
    }
}

impl VectorizedArticles {
    
    pub fn get_debug(&self) -> BTreeMap<&str, ArticleType> {
        self.map.iter().map(|(k, v)| (k.as_str(), v.atype)).collect()
    }

    /// return similar articles based on string distance for article N
    pub fn get_similar_articles(&self, id: &str) -> Vec<String> {
        
        let (article_src, article_type) = match self.map.get(id) {
            Some(s) => (&s.words, s.atype),
            None => return Vec::new(),
        };

        let mut target = Vec::new();
        for (other_key, other) in self.map.iter() {
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

            let dst = strsim::generic_damerau_levenshtein(article_src, &other.words) + penalty;

            target.push((dst, other_key));
        }

        target.sort_by(|a, b| ((a.0) as usize).cmp(&((b.0) as usize)));
        
        target.into_iter().take(10).map(|s| s.1.clone()).collect()
    }
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

    let articles = load_articles(&dir)?;
    let q = articles.langs["de"].vectorize();
    let s = q.get_similar_articles("hexenverfolgung");
    println!("{s:#?}");

    Ok(())
}
