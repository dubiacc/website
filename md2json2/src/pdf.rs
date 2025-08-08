// This module contains the logic for generating PDF files from articles.
// - It converts the article's parsed data into a simple HTML string.
// - It uses the `printpdf` crate to render that HTML into a PDF.
use crate::{par2text, ParsedArticleAnalyzed};
use printpdf::{PdfDocument, GeneratePdfOptions};
use std::collections::BTreeMap;

// A very basic converter from a parsed article to simple HTML for PDF generation.
// This avoids complex CSS and layouts that printpdf might not support well.
fn article_to_simple_html(article: &ParsedArticleAnalyzed) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html><html><head><style>body { font-family: sans-serif; }</style></head><body>");
    html.push_str(&format!("<h1>{}</h1>", article.title));

    if !article.subtitle.is_empty() {
        let subtitle_text = article.subtitle.iter().map(par2text).collect::<String>();
        html.push_str(&format!("<h2>{}</h2>", subtitle_text));
    }

    for p in &article.summary {
        html.push_str(&format!("<p>{}</p>", par2text(p)));
    }

    for section in &article.sections {
        // Using <h2> for all section titles for simplicity in PDF rendering.
        html.push_str(&format!("<h{}>{}</h{}>", section.indent + 1, section.title, section.indent + 1));
        for p in &section.pars {
            html.push_str(&format!("<p>{}</p>", par2text(p)));
        }
    }

    html.push_str("</body></html>");
    html
}

pub fn generate_pdf(article: &ParsedArticleAnalyzed) -> Result<Vec<u8>, String> {
    let html = article_to_simple_html(article);
    let mut warnings = Vec::new();

    // The from_html function in printpdf 0.8.2 expects 5 arguments.
    // We provide empty BTreeMaps for images and fonts as we don't have any to embed.
    // We use default generation options.
    match PdfDocument::from_html(
        &html,
        &BTreeMap::new(), // images
        &BTreeMap::new(), // fonts
        &GeneratePdfOptions::default(),
        &mut warnings
    ) {
        Ok(doc) => {
            if !warnings.is_empty() {
                 // These warnings can be collected and displayed on the WIP page.
                 println!("PDF generation warnings for '{}': {:?}", article.title, warnings);
            }
            // The save method in printpdf 0.8.2 returns a Result<Vec<u8>, Error>
            doc.save(&Default::default()).map_err(|e| e.to_string())
        },
        Err(e) => {
             Err(format!("Failed to generate PDF for '{}': {}", article.title, e))
        }
    }
}
