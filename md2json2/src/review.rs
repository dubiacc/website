// This module will contain the logic for propagating review statuses
// across the translation graph of articles.
// - Find all articles connected through the "translations" field.
// - Propagate `nihil_obstat` and `imprimatur` across the group.
use crate::{AnalyzedArticles, Lang, Slug};
use std::collections::{BTreeSet};

/// Finds all articles connected through the "translations" field using a Breadth-First Search.
fn find_translation_group(
    start_lang: &Lang,
    start_slug: &Slug,
    all_articles: &AnalyzedArticles,
    visited: &mut BTreeSet<(Lang, Slug)>,
) -> BTreeSet<(Lang, Slug)> {
    let mut group = BTreeSet::new();
    let mut queue = vec![(start_lang.clone(), start_slug.clone())];

    // Mark the starting node as visited and add it to the group and queue
    if visited.contains(&(start_lang.clone(), start_slug.clone())) {
        return group; // Already processed this group
    }
    visited.insert((start_lang.clone(), start_slug.clone()));
    group.insert((start_lang.clone(), start_slug.clone()));

    let mut head = 0;
    while head < queue.len() {
        let (lang, slug) = queue[head].clone();
        head += 1;

        if let Some(article) = all_articles.map.get(&lang).and_then(|l| l.get(&slug)) {
            // Check this article's own translations
            for (trans_lang, trans_slug) in &article.translations {
                let key = (trans_lang.clone(), trans_slug.clone());
                if !visited.contains(&key) {
                    visited.insert(key.clone());
                    group.insert(key.clone());
                    queue.push(key);
                }
            }
            // Also need to check other articles that link TO this one
            for (other_lang, other_articles) in &all_articles.map {
                for (other_slug, other_article) in other_articles {
                    if let Some(target_slug) = other_article.translations.get(lang) {
                        if target_slug == &slug {
                            let key = (other_lang.clone(), other_slug.clone());
                            if !visited.contains(&key) {
                                visited.insert(key.clone());
                                group.insert(key.clone());
                                queue.push(key);
                            }
                        }
                    }
                }
            }
        }
    }
    group
}

/// Propagates review status across translation groups and returns an updated map.
pub fn propagate_review_status(articles: &AnalyzedArticles) -> AnalyzedArticles {
    let mut updated_articles = articles.clone();
    let mut visited = BTreeSet::new();

    for (lang, lang_articles) in &articles.map {
        for (slug, _) in lang_articles {
            if visited.contains(&(lang.clone(), slug.clone())) {
                continue;
            }

            let group = find_translation_group(lang, slug, articles, &mut visited);

            // Determine the group's highest review status
            let mut group_has_nihil = false;
            let mut group_has_imprimatur = false;

            for (g_lang, g_slug) in &group {
                if let Some(article) = articles.map.get(g_lang).and_then(|l| l.get(g_slug)) {
                    if article.nihil_obstat.is_some() { group_has_nihil = true; }
                    if article.imprimatur.is_some() { group_has_imprimatur = true; }
                }
            }

            // Apply the highest status to all articles in the group
            for (g_lang, g_slug) in &group {
                if let Some(article) = updated_articles.map.get_mut(g_lang).and_then(|l| l.get_mut(g_slug)) {
                    if group_has_nihil && article.nihil_obstat.is_none() {
                        article.nihil_obstat = Some("Approved".to_string());
                    }
                    if group_has_imprimatur && article.imprimatur.is_none() {
                        article.imprimatur = Some("Approved".to_string());
                    }
                }
            }
        }
    }
    updated_articles
}
