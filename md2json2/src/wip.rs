// This module contains the logic for generating the wip.html page.
// - It will list articles needing review, approval, or translation.
// - It will also display any build warnings.
use crate::{AnalyzedArticles, ArticleStatus, MetaJson, SectionLink, Slug, Lang};
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
            if article.status == ArticleStatus::Prayer {
                continue;
            }
            // WIP articles are rendered in a special wip/ subdirectory
            let link = SectionLink {
                slug: format!("{}/{}", lang, slug = slug),
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
            .map(|item| {
                let (lang, slug) = item.slug.split_once("/").unwrap_or_default();
                format!(
                    "<li><a href='/{lang}/wip/{slug}'>{}</a> (<a href='/{lang}/wip/{slug}.pdf' target='_blank'>Preview PDF</a>)</li>",
                    item.title,
                    lang = lang,
                    slug = slug
                )
            })
            .collect::<String>();
        format!("<h2 class='filterable-header'>{}</h2><ul class='filterable'>{}</ul>", title, list_items)
    };

    let render_translation_list = |title: &str, items: &BTreeMap<String, Vec<String>>| -> String {
        if items.is_empty() { return String::new(); }
        let list_items = items
            .iter()
            .map(|(title, langs)| format!("<li>{} → [{}]</li>", title, langs.join(", ")))
            .collect::<String>();
        format!("<h2 class='filterable-header'>{}</h2><ul class='filterable'>{}</ul>", title, list_items)
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
    <meta name="viewport" content="width=device-width, initial-scale=1">
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
        #searchInput {{ width: 100%; padding: 10px; margin-bottom: 20px; font-size: 1em; border-radius: 5px; border: 1px solid #ddd; }}
    </style>
</head>
<body>
    <h1>Work in Progress</h1>
    <input type="text" id="searchInput" onkeyup="searchFunction()" placeholder="Search for articles...">
    {}
    {}
    {}
    {}
    <script>
    function searchFunction() {{
        var input, filter, ul, li, a, i, txtValue;
        input = document.getElementById('searchInput');
        filter = input.value.toUpperCase();
        var lists = document.getElementsByClassName('filterable');
        for (var j = 0; j < lists.length; j++) {{
            ul = lists[j];
            li = ul.getElementsByTagName('li');
            var header = ul.previousElementSibling;
            var visibleItems = 0;
            for (i = 0; i < li.length; i++) {{
                a = li[i].getElementsByTagName("a")[0];
                txtValue = a.textContent || a.innerText;
                if (txtValue.toUpperCase().indexOf(filter) > -1) {{
                    li[i].style.display = "";
                    visibleItems++;
                }} else {{
                    li[i].style.display = "none";
                }}
            }}
            if (visibleItems > 0) {{
                header.style.display = "";
                ul.style.display = "";
            }} else {{
                header.style.display = "none";
                ul.style.display = "none";
            }}
        }}
    }}
    </script>
</body>
</html>
"#,
    render_list("Needs Nihil Obstat", needs_nihil_obstat),
    render_list("Needs Imprimatur", needs_imprimatur),
    render_translation_list("Needs Translation", &needs_translation),
    warnings_html
    )
}
