use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use crate::{ParsedArticleAnalyzed, get_string, MetaJson, minify, get_root_href};
use crate::{head, header_navigation, link_tags, table_of_contents, page_desciption, page_metadata};
use crate::{body_abstract, body_content, body_noscript, footnotes, bibliography, body_footer};

type Lang = String;
type Author = String;
type Slug = String;

#[derive(Debug, Default)]
pub struct LoadedDocuments {
    pub langs: BTreeMap<Lang, BTreeMap<Author, BTreeMap<Slug, String>>>,
}

#[derive(Debug, Default)]
pub struct AnalyzedDocuments {
    pub map: BTreeMap<Lang, BTreeMap<Author, BTreeMap<Slug, ParsedArticleAnalyzed>>>,
}

/// Load documents from the /docs directory
pub fn load_documents(dir: &Path) -> Result<LoadedDocuments, String> {
    let mut langs = BTreeMap::new();
    
    let entries = walkdir::WalkDir::new(dir)
        .max_depth(5)
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.map_err(|e| e.to_string()).ok()?;
            let path = entry.path();
            
            // Skip index.md files for now (we'll handle them separately)
            let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if fname == "index.md" || fname == "README.md" {
                return None;
            }
            
            // Only process .md files
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                return None;
            }
            
            // Extract language, author, and slug
            let file_name = path.file_name()?.to_str()?;
            let slug = file_name.strip_suffix(".md")?.to_string();
            
            let author = path.parent()?.file_name()?.to_str()?.to_string();
            let lang = path.parent()?.parent()?.file_name()?.to_str()?.to_string();
            
            let contents = std::fs::read_to_string(&path).ok()?;
            
            Some((lang, author, slug, contents))
        })
        .collect::<Vec<_>>();

    for (lang, author, slug, contents) in entries {
        langs
            .entry(lang)
            .or_insert_with(|| BTreeMap::default())
            .entry(author)
            .or_insert_with(|| BTreeMap::default())
            .insert(slug, contents);
    }

    Ok(LoadedDocuments { langs })
}

/// Process documents - similar to article processing but for documents
pub fn process_documents(documents: &LoadedDocuments) -> Result<AnalyzedDocuments, String> {
    let mut analyzed = AnalyzedDocuments::default();
    
    for (lang, authors) in &documents.langs {
        for (author, slugs) in authors {
            for (slug, content) in slugs {
                let mut parsed = crate::parse_article(content);
                
                // If no author is specified in the markdown, add the directory author
                if parsed.authors.is_empty() {
                    parsed.authors.push(author.clone());
                }
                
                let analyzed_doc = ParsedArticleAnalyzed {
                    title: parsed.title.clone(),
                    date: parsed.date.clone(),
                    tags: parsed.tags.clone(),
                    authors: parsed.authors.clone(),
                    sha256: parsed.sha256.clone(),
                    img: parsed.img.clone(),
                    subtitle: parsed.summary.clone(),
                    summary: parsed.article_abstract.clone(),
                    sections: parsed.sections.clone(),
                    similar: Vec::new(), // No similarities for documents
                    backlinks: Vec::new(),
                    bibliography: parsed.get_bibliography(),
                    footnotes: parsed.footnotes.clone(),
                    // Add default values for new fields
                    nihil_obstat: None,
                    imprimatur: None,
                    translations: BTreeMap::new(),
                    status: crate::ArticleStatus::default(),
                    src: content.to_string(),
                };
                
                analyzed
                    .map
                    .entry(lang.clone())
                    .or_insert_with(|| BTreeMap::default())
                    .entry(author.clone())
                    .or_insert_with(|| BTreeMap::default())
                    .insert(slug.clone(), analyzed_doc);
            }
        }
    }
    
    Ok(analyzed)
}

/// Load document index files (for the document index page)
pub fn load_document_indices(dir: &Path) -> Result<BTreeMap<Lang, String>, String> {
    let mut indices = BTreeMap::new();
    
    let entries = walkdir::WalkDir::new(dir)
        .max_depth(2) // Just the language level
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.map_err(|e| e.to_string()).ok()?;
            let path = entry.path();
            
            if path.file_name().and_then(|s| s.to_str()) == Some("index.md") {
                let lang = path.parent()?.file_name()?.to_str()?.to_string();
                let content = std::fs::read_to_string(path).ok()?;
                Some((lang, content))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    
    for (lang, content) in entries {
        indices.insert(lang, content);
    }
    
    Ok(indices)
}

/// Generate document HTML
pub fn document2html(
    lang: &str,
    author: &str,
    slug: &str,
    doc: &ParsedArticleAnalyzed,
    meta: &MetaJson,
) -> Result<String, String> {
    // Similar to article2html but with adjusted paths and without similarity search
    static HTML: &str = include_str!("../../templates/lorem.html");
    
    let title_id = format!("{}-docs-{}-{}", lang, author, slug);
    
    let html = HTML.replace(
        "<!-- HEAD_TEMPLATE_HTML -->",
        &head(doc, lang, &title_id, meta, true, author, slug)?,
    );
    let html = html.replace(
        "<!-- HEADER_NAVIGATION -->",
        &header_navigation(lang, true, meta)?,
    );
    let html = html.replace("<!-- LINK_TAGS -->", &link_tags(lang, &doc.tags, meta)?);
    let html = html.replace("<!-- TOC -->", &table_of_contents(lang, doc, meta)?);
    let html = html.replace(
        "<!-- PAGE_DESCRIPTION -->",
        &page_desciption(lang, doc, meta)?,
    );
    let html = html.replace("<!-- PAGE_METADATA -->", &page_metadata(lang, doc, meta)?);
    let html = html.replace(
        "<!-- BODY_ABSTRACT -->",
        &body_abstract(lang, slug, doc.is_prayer(), &doc.summary),
    );
    let html = html.replace("<!-- BODY_CONTENT -->", &body_content(lang, slug, &doc.sections, meta)?);
    
    // No donate, similar sections for documents
    let html = html.replace("<!-- DONATE -->", "");
    let html = html.replace("<!-- SIMILARS -->", "");
    
    let html = html.replace("<!-- BODY_NOSCRIPT -->", &body_noscript());
    let html = html.replace("<!-- FOOTNOTES -->", &footnotes(lang, doc, meta)?);
    let html = html.replace("<!-- BACKLINKS -->", &crate::backlinks(lang, doc, meta)?);
    let html = html.replace("<!-- BIBLIOGRAPHY -->", &bibliography(lang, doc, meta)?);
    let html = html.replace("<!-- BODY_FOOTER -->", &body_footer(lang, doc, meta)?);
    
    // Standard replacements
    let skip = get_string(meta, lang, "page-smc")?;
    let html = html.replace("$$SKIP_TO_MAIN_CONTENT$$", &skip);
    let contact = crate::get_special_page_link(lang, "about", meta)?;
    let root_href = get_root_href();
    let docs_folder = get_string(meta, lang, "special-docs-path")?;
    let special_about_path = get_string(meta, lang, "special-about-path")?;
    let special_about_title = get_string(meta, lang, "special-about-title")?;

    let html = html.replace("$$SPECIAL_ABOUT_PATH$$", &special_about_path);
    let html = html.replace("$$SPECIAL_ABOUT_TITLE$$", &special_about_title);
    let html = html.replace("$$CONTACT_URL$$", &contact);
    let html = html.replace("$$TITLE$$", &doc.title);
    let html = html.replace("$$TITLE_ID$$", &title_id);
    let html = html.replace("$$LANG$$", lang);
    let html = html.replace("$$SLUG$$", slug);
    let html = html.replace("$$ROOT_HREF$$", root_href);
    let html = html.replace(
        "$$PAGE_HREF$$",
        &format!("{}/{}/{}/{}/{}", root_href, lang, docs_folder, author, slug),
    );
    
    Ok(html)
}

pub fn get_document_index_content(
    lang: &str,
    documents: &AnalyzedDocuments,
    meta: &MetaJson,
) -> Result<String, String> {
    // Create a page that lists all documents by author
    let mut content = String::new();
    
    let docs_folder = get_string(meta, lang, "special-docs-path")?;

    // Group documents by author
    if let Some(lang_docs) = documents.map.get(lang) {
        for (author, docs) in lang_docs {
            // Create section for each author
            let author_name = meta.authors.get(author)
                .map(|a| a.displayname.clone())
                .unwrap_or_else(|| author.clone());
                
            content.push_str(&format!("<h2 id='author-{}' class='heading-level-1'>{}</h2>", author, author_name));
            content.push_str("<ul class='list'>");
            
            // Add all documents for this author
            for (slug, doc) in docs {
                content.push_str(&format!(
                    "<li class='link-modified-recently-list-item dark-mode-invert'><p class='in-list first-graf block'><a href='/{}/{}/{}/{}' class='link-annotated link-page'>{}</a></p></li>",
                    lang, docs_folder, author, slug, doc.title
                ));
            }
            
            content.push_str("</ul>");
        }
    }

    Ok(content)
}
