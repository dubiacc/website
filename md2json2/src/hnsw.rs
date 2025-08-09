use hnsw_rs::hnsw::{Hnsw, HnswParams};
use hnsw_rs::api::AnnT;
use std::collections::{BTreeMap, HashMap};
use rayon::prelude::*;

const EMBEDDING_DIM: usize = 512;
const MAX_VOCAB_SIZE: usize = 10000;

#[derive(Debug, Clone)]
struct ArticleEmbedding {
    vector: Vec<f32>,
    slug: String,
    atype: ArticleType,
    title: String,
}

struct SimilarityIndex {
    hnsw: Hnsw<f32, usize>,
    articles: Vec<ArticleEmbedding>,
    vocab: HashMap<u32, usize>,
}

impl SimilarityIndex {
    fn build(articles: &BTreeMap<String, VectorizedArticle>) -> Self {
        let vocab = Self::build_vocab(articles);
        let embeddings = Self::vectorize_articles(articles, &vocab);
        let hnsw = Self::build_index(&embeddings);
        
        Self { hnsw, articles: embeddings, vocab }
    }

    fn build_vocab(articles: &BTreeMap<String, VectorizedArticle>) -> HashMap<u32, usize> {
        let mut token_counts: HashMap<u32, usize> = HashMap::new();
        
        articles.values().for_each(|article| {
            article.words.iter().for_each(|&token| {
                *token_counts.entry(token).or_insert(0) += 1;
            });
        });

        token_counts
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .collect::<Vec<_>>()
            .into_iter()
            .enumerate()
            .take(MAX_VOCAB_SIZE)
            .map(|(idx, (token, _))| (token, idx))
            .collect()
    }

    fn vectorize_articles(
        articles: &BTreeMap<String, VectorizedArticle>,
        vocab: &HashMap<u32, usize>,
    ) -> Vec<ArticleEmbedding> {
        articles
            .par_iter()
            .map(|(slug, article)| {
                let vector = Self::tokens_to_tfidf(&article.words, vocab, articles.len());
                ArticleEmbedding {
                    vector,
                    slug: slug.clone(),
                    atype: article.atype,
                    title: article.parsed.title.clone(),
                }
            })
            .collect()
    }

    fn tokens_to_tfidf(tokens: &[u32], vocab: &HashMap<u32, usize>, total_docs: usize) -> Vec<f32> {
        let mut tf = vec![0.0; EMBEDDING_DIM.min(vocab.len())];
        let mut token_counts = HashMap::new();
        
        tokens.iter().for_each(|&token| {
            *token_counts.entry(token).or_insert(0) += 1;
        });

        let doc_len = tokens.len() as f32;
        
        for (&token, &count) in &token_counts {
            if let Some(&vocab_idx) = vocab.get(&token) {
                if vocab_idx < tf.len() {
                    let tf_score = count as f32 / doc_len;
                    let idf_score = (total_docs as f32 / (count + 1) as f32).ln();
                    tf[vocab_idx] = tf_score * idf_score;
                }
            }
        }

        Self::normalize_vector(&tf)
    }

    fn normalize_vector(vec: &[f32]) -> Vec<f32> {
        let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            vec.iter().map(|x| x / norm).collect()
        } else {
            vec!{0.0; vec.len()}
        }
    }

    fn build_index(embeddings: &[ArticleEmbedding]) -> Hnsw<f32, usize> {
        let params = HnswParams::default();
        let mut hnsw = Hnsw::new(params, EMBEDDING_DIM.min(embeddings.first().map_or(0, |e| e.vector.len())));
        
        embeddings.iter().enumerate().for_each(|(idx, embedding)| {
            hnsw.insert((&embedding.vector, idx));
        });

        hnsw
    }

    fn find_similar(&self, target_idx: usize, k: usize) -> Vec<(usize, f32)> {
        let target = &self.articles[target_idx];
        let neighbors = self.hnsw.search(&target.vector, k + 1, 50);
        
        neighbors
            .into_iter()
            .filter(|(idx, _)| *idx != target_idx)
            .map(|(idx, dist)| {
                let penalty = self.calculate_type_penalty(target.atype, self.articles[idx].atype);
                (idx, dist + penalty)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .take(k)
            .collect()
    }

    fn calculate_type_penalty(&self, source: ArticleType, target: ArticleType) -> f32 {
        match (source, target) {
            (ArticleType::Prayer, ArticleType::Prayer)
            | (ArticleType::Tract, ArticleType::Tract)
            | (ArticleType::Question, ArticleType::Question) => 0.0,
            (ArticleType::Prayer, _) | (_, ArticleType::Prayer) => f32::INFINITY,
            _ => 0.1,
        }
    }
}

#[cfg(feature = "external")]
fn get_similar_articles(
    s: &VectorizedArticle,
    id: &str,
    map: &BTreeMap<String, VectorizedArticle>,
) -> Vec<SectionLink> {
    let index = SimilarityIndex::build(map);
    
    let target_idx = index.articles
        .iter()
        .position(|article| article.slug == id)?;
    
    let similar = index.find_similar(target_idx, 10);
    
    similar
        .into_iter()
        .filter_map(|(idx, _)| {
            let article = &index.articles[idx];
            if article.vector.iter().any(|&x| x.is_infinite()) {
                return None;
            }
            Some(SectionLink {
                slug: article.slug.clone(),
                title: article.title.clone(),
                id: None,
            })
        })
        .collect()
}

impl VectorizedArticles {
    pub fn analyze_with_hnsw(&self) -> AnalyzedArticles {
        AnalyzedArticles {
            map: self
                .map
                .par_iter()
                .map(|(lang, articles)| {
                    let index = SimilarityIndex::build(articles);
                    
                    let analyzed = articles
                        .iter()
                        .enumerate()
                        .map(|(idx, (slug, vectorized))| {
                            println!("finding similar articles for {slug}...");
                            
                            let similar = index
                                .find_similar(idx, 10)
                                .into_iter()
                                .filter_map(|(other_idx, _)| {
                                    let other = &index.articles[other_idx];
                                    Some(SectionLink {
                                        slug: other.slug.clone(),
                                        title: other.title.clone(),
                                        id: None,
                                    })
                                })
                                .collect();

                            let backlinks = self.collect_backlinks(lang, slug, vectorized);

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
                                    summary: normalize_first_char(&vectorized.parsed.article_abstract),
                                    sections: vectorized.parsed.sections.clone(),
                                    similar,
                                    backlinks,
                                    bibliography: vectorized.parsed.get_bibliography(),
                                    footnotes: vectorized.parsed.footnotes.clone(),
                                    nihil_obstat: vectorized.parsed.nihil_obstat.clone(),
                                    imprimatur: vectorized.parsed.imprimatur.clone(),
                                    translations: vectorized.parsed.translations.clone(),
                                    status: vectorized.status,
                                    src: vectorized.parsed.src.clone(),
                                },
                            )
                        })
                        .collect();

                    (lang.clone(), analyzed)
                })
                .collect(),
        }
    }

    fn collect_backlinks(&self, lang: &str, slug: &str, vectorized: &VectorizedArticle) -> Vec<SectionLink> {
        self.map
            .iter()
            .flat_map(|(lang2, v2)| {
                v2.iter().filter_map(move |(slug2, vectorized2)| {
                    if lang2 != lang || slug2 == slug {
                        return None;
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
            .collect()
    }
}