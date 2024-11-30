use std::{collections::BTreeMap, path::Path};
use comrak::{arena_tree::NodeEdge, nodes::ListType};
use serde_derive::{Deserialize, Serialize};

fn main() -> Result<(), String> {

    let dir = get_dir::get_dir_by_target(get_dir::Target { 
        name: "articles", 
        ty: get_dir::TargetType::Dir,
    }).map_err(|_| "cannot find /articles dir in current path")?;

    let entries = walkdir::WalkDir::new(dir)
    .max_depth(5)
    .into_iter()
    .filter_map(|entry| {
        let entry = entry.map_err(|e| e.to_string()).ok()?;
        let entry = entry.path();
        if entry.file_name().and_then(|s| s.to_str()) == Some("index.md") {
            Some(entry.to_path_buf())
        } else {
            None
        }
    }).collect::<Vec<_>>();

    println!("");
    println!("indexing {} files", entries.len());
    println!("");

    for index_md in entries.iter() {

        let parent = index_md.parent().and_then(|s| s.parent())
        .and_then(|s| s.file_name().and_then(|q| q.to_str())).unwrap_or("");
        
        let file = index_md.parent()
        .and_then(|s| s.file_name().and_then(|q| q.to_str())).unwrap_or("");

        println!("indexing {parent}/{file}");

        let file_loaded = std::fs::read_to_string(&index_md)
            .map_err(|e| format!("error loading {}: {e}", index_md.display()))?;
        
        let file_parsed = parse_file(&file_loaded)
            .map_err(|e| format!("error parsing {}: {e}", index_md.display()))?;
        
        let file_parsed = file_parsed.optimize();
        let sections = split_sections(&file_parsed.content);
        let images = file_parsed.transcode_image_links(index_md)?;
        
        MdFile {
            images,
            title: sections.title,
            date: file_parsed.config.date.clone(),
            tags: file_parsed.config.tags.clone(),
            authors: file_parsed.config.authors.clone(),
            sections: sections.sections,
            tagline: sections.tagline,
            img: sections.img,
            summary: sections.summary,
        }.save_to_json(&index_md)?;
    }

    println!("");

    Ok(())
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct MdFile {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    date: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    authors: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    images: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tagline: Vec<MdNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    img: Option<Link>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    summary: Vec<Vec<MdNode>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sections: Vec<MdFileSection>,
}

impl MdFile {
    pub fn save_to_json(&self, index_md_path: &Path) -> Result<(), String> {
        
        let mut parent_dir = index_md_path.to_path_buf();
        if parent_dir.is_file() {
            parent_dir.pop();
        }

        let fs = parent_dir.join("index.md.json");
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        
        std::fs::write(&fs, json)
        .map_err(|e| format!("{}: {e}", fs.display()))
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct MdFileSection {
    header: String,
    level: usize,
    paragraphs: Vec<Vec<MdNode>>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct Config {
    #[serde(default)]
    title: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    authors: Vec<String>,
}

fn transcode_image_to_avif(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Cursor;
    let im = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let im = im.resize(1024, 1024, image::imageops::FilterType::Triangle);
    let mut target = Cursor::new(Vec::<u8>::new());
    let _ = im.write_to(&mut target, image::ImageFormat::Avif).map_err(|e| e.to_string())?;
    Ok(target.into_inner())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "lowercase")]
enum MdNode {
    Text { 
        text: String, 
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        context: Vec<Context>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        link: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    Code { 
        lines: Vec<String>, 
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        context: Vec<Context>,
    },
    LineBreak,
}

impl MdNode {

    pub fn get_context(&self) -> Vec<Context> {
        match self {
            MdNode::Text { context, .. } | MdNode::Code { context, .. } => context.clone(),
            MdNode::LineBreak => Vec::new()
        }
    }

    pub fn get_link(&self) -> Option<Link> {

        let (title, link) = match self {
            MdNode::Text { link, text, .. } => (link, text),
            _ => return None,
        };

        if !self.get_context().iter().any(|s| *s == Context::Link) {
            return None;
        }

        Some(Link {
            href: link.clone(),
            title: title.clone().unwrap_or_default(),
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Context {
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,

    Link,

    Bold,
    Italic,
    Underline,
    Subscript,
    Superscript,
    Strikethrough,

    BlockQuote,
    UnorderedList,
    OrderedList,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Link {
    href: String,
    title: String,
}

impl Context {
    pub fn is_heading(&self) -> bool {
        use self::Context::*;
        match self {
            H1 |
            H2 |
            H3 |
            H4 |
            H5 |
            H6 => true,
            _ => false,
        }
    }

    pub fn get_header_level(&self) -> usize {
        use self::Context::*;
        match self {
            H1 => 1,
            H2 => 2,
            H3 => 3,
            H4 => 4,
            H5 => 5,
            H6 => 6,
            _ => 7,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct JsonOutput {
    config: Config,
    content: Vec<MdNode>,
}

impl JsonOutput {
    fn optimize(&self) -> Self {

        let mut last_text = String::new();
        let mut last_context = Vec::new();
        let mut newitems = Vec::new();
        let mut last_link = None;
        let mut last_title = None;

        for i in self.content.iter() {
            match i {
                MdNode::Code { .. } => newitems.push(i.clone()),
                MdNode::LineBreak => {
                    newitems.push(MdNode::Text {
                        text: last_text.split_whitespace().collect::<Vec<_>>().join(" "), 
                        context: last_context,
                        link: None,
                        title: None,
                    });
                    last_text.clear();
                    last_context = Vec::new();
                    newitems.push(MdNode::LineBreak);
                },
                MdNode::Text { 
                    text, 
                    context, 
                    link, 
                    title 
                } => {

                    if text.is_empty() {
                        continue;
                    }

                    if *context == last_context {
                        last_text.push_str(&(String::from(" ") + text + " "));
                        last_link = link.clone();
                        last_title = title.clone();
                    } else if *context != last_context && last_text.is_empty() {
                        last_text.push_str(&(String::from(" ") + text + " "));
                        last_link = link.clone();
                        last_title = title.clone();
                    } else if *context != last_context && !last_text.is_empty() {
                        newitems.push(MdNode::Text { 
                            text: last_text.split_whitespace().collect::<Vec<_>>().join(" "), 
                            context: last_context,
                            link: last_link.clone(),
                            title: last_title.clone(),
                        });
                        last_text.clear();
                        last_link = None;
                        last_title = None;
                        last_text.push_str(&(String::from(" ") + text + " "));
                    }
                    last_context = context.clone();
                }
            }
        }
    
        if !last_text.is_empty() {
            newitems.push(MdNode::Text { 
                text: last_text, 
                context: last_context,
                link: last_link,
                title: last_title,
            });
        }

        Self {
            content: newitems,
            config: self.config.clone(),
        }
    }

    // transcodes and rescales the images from [png, jpeg, webp, bmp, ...] to avif
    fn transcode_image_links(&self, index_md: &Path) -> Result<BTreeMap<String, String>, String> {
        let mut parent_dir = index_md.to_path_buf();
        if parent_dir.is_file() {
            parent_dir.pop();
        }

        let mut map = BTreeMap::new();
        for s in self.content.iter() {
            match s {
                MdNode::Text { link, .. }=> {
                    let href = link.as_deref().unwrap_or("");
                    if href.is_empty() || href.contains("://") {
                        continue;
                    }
                    let mut image_path_path = parent_dir.join(href);
                    let image_path = image_path_path.to_string_lossy().to_string();
                    if map.contains_key(&image_path) {
                        continue;
                    }

                    image_path_path.set_extension("avif");

                    if image_path_path.exists() {
                        let parent = parent_dir.to_string_lossy().to_string() + "/";
                        let source = image_path.clone().replace(&parent, "");
                        let target = image_path_path.to_string_lossy().to_string().replace(&parent, "");
                        map.insert(source, target);
                        continue;
                    }

                    let file = std::fs::read(&image_path)
                    .map_err(|e| format!("{}: cannot find image {href:?}: {e}", parent_dir.display()))?;

                    let transcoded = transcode_image_to_avif(&file)
                    .map_err(|e| format!("{}: cannot transcode image {href:?}: {e}", parent_dir.display()))?;

                    std::fs::write(&image_path_path, transcoded)
                    .map_err(|e| format!("{}: cannot write transcoded image {href:?}: {e}", parent_dir.display()))?;

                    let _ = std::fs::remove_file(&image_path);

                    map.insert(image_path, image_path_path.to_string_lossy().to_string());
                },
                _ => { },
            }
        }

        // replace image links in index_md file for the next time
        if let Ok(mut file) = std::fs::read_to_string(&index_md) {
            for (k, v) in map.iter() {
                file = file.replace(&format!("({k})"), &format!("({v})"));
            }
            let _ = std::fs::write(&index_md, file);
        }

        Ok(map)
    }
}

fn parse_file(input: &str) -> Result<JsonOutput, String> {

    use comrak::{Arena, parse_document, Options};
    use comrak::nodes::NodeValue;

    let arena = Arena::new();
    let root = parse_document(&arena,input,&Options::default());

    let items = root.traverse()
    .filter_map(|q| {
        
        let (q2, opening) = match q {
            NodeEdge::Start(s) => {
                (s.data.borrow().clone(), true)
            },
            NodeEdge::End(s) => {
                (s.data.borrow().clone(), false)
            },
        };

        Some((q2, opening))
    }).collect::<Vec<_>>();

    let mut config = Config::default();
    let mut target = Vec::new();
    let mut text = String::new();
    let mut context = Vec::new();
    let mut last_link = None;
    let mut last_title = None;

    for (i, opening) in items {
        match i.value {
            NodeValue::List(nl) => {
                let item = match nl.list_type {
                    ListType::Bullet => Context::UnorderedList,
                    ListType::Ordered => Context::OrderedList,
                };
                if opening {
                    context.push(item);
                } else {
                    loop {
                        if context.is_empty() || context.pop() == Some(item.clone()) {
                            break;
                        }
                    }
                }
            },
            NodeValue::CodeBlock(code) => {
                if opening {
                    let c = code.literal.lines().map(String::from).collect::<Vec<_>>().join(" ");
                    let res = serde_json::from_str::<Config>(&c);
                    if let Ok(c) = res {
                        config = c;
                    } else {
                        target.push(MdNode::Code { lines: code.literal.lines().map(String::from).collect(), context: context.clone() });
                    }
                }
            },
            NodeValue::Paragraph => {
                if !opening {
                    if !text.is_empty() {
                        target.push(MdNode::Text { 
                            text: text.clone(), 
                            context: context.clone(),
                            link: last_link.clone(),
                            title: last_title.clone(),
                        });
                        last_link = None;
                        last_title = None;
                        text = String::new();
                        context = Vec::new();
                    }
                    target.push(MdNode::LineBreak);
                }
            },
            NodeValue::Text(t) => {
                if opening {
                    text.push_str(&t);
                } else {
                    target.push(MdNode::Text { 
                        text: text.clone(), 
                        context: context.clone(),
                        link: last_link.clone(),
                        title: last_title.clone(),
                    });
                    last_link = None;
                    last_title = None;
                    text = String::new();
                }
            },
            NodeValue::Heading(h) => {
                let item = match h.level {
                    0 | 1 => Context::H1,
                    2 => Context::H2,
                    3 => Context::H3,
                    4 => Context::H4,
                    5 => Context::H5,
                    6 => Context::H6,
                    _ => Context::Bold,
                };
                if opening {
                    context.push(item);
                } else {
                    loop {
                        if context.is_empty() || context.pop() == Some(item.clone()) {
                            break;
                        }
                    }
                }
            },
            NodeValue::Emph |
            NodeValue::Strong |
            NodeValue::Strikethrough |
            NodeValue::Superscript |
            NodeValue::Subscript |
            NodeValue::Underline |
            NodeValue::BlockQuote => {

                let ctx = match i.value {
                    NodeValue::Emph => Context::Italic,
                    NodeValue::Strong => Context::Bold,
                    NodeValue::Strikethrough => Context::Strikethrough,
                    NodeValue::Superscript => Context::Superscript,
                    NodeValue::Subscript => Context::Subscript,
                    NodeValue::Underline => Context::Underline,
                    NodeValue::BlockQuote => Context::BlockQuote,
                    _ => break,
                };

                if opening {
                    context.push(ctx);
                } else {
                    loop {
                        if context.is_empty() || context.pop() == Some(ctx) {
                            break;
                        }
                    }
                }
            },
            NodeValue::Link(node_link) | NodeValue::Image(node_link) => {
                let item = Context::Link;
                
                if opening {
                    context.push(item.clone());
                    last_link = Some(node_link.url.clone());
                    last_title = Some(node_link.title.clone());
                } else {
                    last_link = None;
                    last_title = None;
                    loop {
                        if context.is_empty() || context.pop() == Some(item.clone()) {
                            break;
                        }
                    }
                }
            },
            _ => { },
        }
    }
    
    Ok(JsonOutput {
        config,
        content: target,
    })
}

fn is_heading(c: &[Context]) -> bool {
    c.iter().any(|s| s.is_heading())
}

struct SplitSections {
    tagline: Vec<MdNode>,
    title: String,
    img: Option<Link>,
    summary: Vec<Vec<MdNode>>,
    sections: Vec<MdFileSection>,
}

fn split_sections(s: &[MdNode]) -> SplitSections {

    let mut s = s.to_vec();
    s.reverse();

    let splits = s
        .split_inclusive(|s| is_heading(&s.get_context()))
        .map(|q| q.iter().rev().collect::<Vec<_>>())
        .collect::<Vec<_>>();

    let mut s = splits.iter().map(|q| {

        let mut level = 7;
        let header = q.iter()
        .filter_map(|q| match q {
            MdNode::Text { text, context, .. } => if is_heading(&context) { 
                level = level.min(context.iter().map(|q| q.get_header_level()).min().unwrap_or(7));
                Some(text.clone()) 
            } else { None },
            _ => None,
        }).collect::<Vec<_>>().join(" ");

        let paragraphs = q
        .split(|s| **s == MdNode::LineBreak)
        .filter_map(|s| {
            
            let q = s.iter()
            .filter(|q| !is_heading(&q.get_context()))
            .map(|q| (*q).clone())
            .collect::<Vec<_>>();

            if q.is_empty() {
                None
            } else {
                Some(q)
            }
        })
        .collect::<Vec<_>>();

        MdFileSection {
            header,
            level,
            paragraphs,
        }
    }).collect::<Vec<_>>();

    s.reverse();

    let mut tagline = Vec::new();
    let mut title = String::new();
    let mut summary = Vec::new();
    let mut sections = Vec::new();
    let mut img = None;

    for s in s.iter() {

        if s.header.trim().is_empty() {

            for p in s.paragraphs.iter() {
                for p in p.iter() {
                    match p.get_link() {
                        Some(s) => img = Some(s.clone()),
                        None => tagline.push(p.clone()),
                    }
                }
            }

            continue;
        }

        if s.level == 1 {
            title.push_str(&s.header);
            summary.extend(s.paragraphs.clone());
            continue;
        }

        sections.push(s.clone());

    }

    SplitSections {
        tagline,
        img,
        title,
        summary,
        sections,
    }

}