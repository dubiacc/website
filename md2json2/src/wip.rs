// This module contains the logic for generating the wip.html page.
// - It will list articles needing review, approval, or translation.
// - It will also display any build warnings.
use crate::{AnalyzedArticles, MetaJson, SectionLink, Slug, Lang};
use std::collections::{BTreeMap, BTreeSet};

pub fn generate_wip_page(
    all_articles: &AnalyzedArticles,
    meta: &MetaJson,
    warnings: &[String],
    wip_url_base: &str,
) -> String {
    let mut needs_nihil_obstat = Vec::new();
    let mut needs_imprimatur = Vec::new();
    let mut needs_translation: BTreeMap<String, Vec<String>> = BTreeMap::new();

    let all_langs: BTreeSet<_> = meta.strings.keys().cloned().collect();

    for (lang, articles) in &all_articles.map {
        for (slug, article) in articles {
            // WIP articles are rendered in a special wip/ subdirectory
            let link = SectionLink {
                slug: format!("{}/{}/wip/{}.html", wip_url_base, lang, slug),
                title: article.title.clone(),
                id: None,
            };

            if article.imprimatur.is_some() {
                // This article is approved. Check if it needs translation.
                let mut missing_langs = Vec::new();
                let existing_langs: BTreeSet<_> = article.translations.keys().cloned().collect();

                for target_lang in &all_langs {
                    // Don't translate to self
                    if target_lang == lang { continue; }

                    // Check if a translation exists for the target language
                    if !existing_langs.contains(target_lang) {
                        missing_langs.push(target_lang.clone());
                    }
                }

                if !missing_langs.is_empty() {
                    let key = format!("{} (from {})", article.title, lang);
                    needs_translation.insert(key, missing_langs);
                }

            } else if article.nihil_obstat.is_some() {
                needs_imprimatur.push(link);
            } else {
                needs_nihil_obstat.push(link);
            }
        }
    }

    let render_list = |title: &str, items: Vec<SectionLink>| -> String {
        if items.is_empty() { return String::new(); }
        let list_items = items
            .iter()
            .map(|item| format!("<li><a href='{}'>{}</a></li>", item.slug, item.title))
            .collect::<String>();
        format!("<h2>{}</h2><ul>{}</ul>", title, list_items)
    };

    let render_translation_list = |title: &str, items: &BTreeMap<String, Vec<String>>| -> String {
        if items.is_empty() { return String::new(); }
        let list_items = items
            .iter()
            .map(|(title, langs)| format!("<li>{} → [{}]</li>", title, langs.join(", ")))
            .collect::<String>();
        format!("<h2>{}</h2><ul>{}</ul>", title, list_items)
    };

    let warnings_html = if !warnings.is_empty() {
        format!("<h2>Build Warnings</h2><pre><code>{}</code></pre>", warnings.join("\n"))
    } else {
        String::new()
    };

    format!(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Work in Progress</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; line-height: 1.6; margin: 2em; background-color: #fdfdfd; color: #1a1a1a; }}
        h1, h2 {{ border-bottom: 1px solid #e0e0e0; padding-bottom: 5px; color: #333; }}
        h1 {{ font-size: 2em; }}
        h2 {{ font-size: 1.5em; }}
        ul {{ list-style-type: disc; padding-left: 20px; }}
        li {{ margin-bottom: 0.5em; }}
        a {{ color: #0077cc; text-decoration: none; }}
        a:hover {{ text-decoration: underline; }}
        pre {{ background-color: #f0f0f0; padding: 1em; border: 1px solid #ddd; border-radius: 5px; white-space: pre-wrap; word-wrap: break-word; font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, Courier, monospace; }}
        code {{ font-size: 0.95em; }}
    </style>
</head>
<body>
    <h1>Work in Progress</h1>
    {}
    {}
    {}
    {}
</body>
</html>
"#,
    render_list("Needs Nihil Obstat", needs_nihil_obstat),
    render_list("Needs Imprimatur", needs_imprimatur),
    render_translation_list("Needs Translation", &needs_translation),
    warnings_html
    )
}
