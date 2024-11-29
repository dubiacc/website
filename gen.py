import sys, base64, os, json

# Returns if the script is run in a production 
# environment ("with --production")
def is_prod():
    l = len(sys.argv)
    if l > 1:
        return sys.argv[1] == "--production"
    else:
        return False
    
# Reads a text file as a string
def read_file(path):
    text_file = open(path, 'r')
    text_file_contents = text_file.read()
    text_file.close()
    return text_file_contents

# Reads a binary file as a base64 string
def read_file_base64(path):
    encoded_string = ""
    with open(path, "rb") as image_file:
        encoded_string = base64.b64encode(image_file.read()).decode()
    return encoded_string

# Writes a text string to a file
def write_file(string, path):
    text_file = open(path, "w+", newline='')
    text_file.write(string)
    text_file.close()

# Chunks an array into chunks of maximum n items
def chunks(lst, n):
    """Yield successive n-sized chunks from lst."""
    for i in range(0, len(lst), n):
        yield lst[i:i + n]

# Loads templates from the /templates folder, so while generating articles
# we don't have to touch the filesystem. Also does some preliminary CSS inlining.
def load_templates():
    templates = {}
    templates["index"] = read_file("./templates/lorem.html")
    inlined_styles_light = read_file("./templates/inlined-styles.css")
    inlined_styles_dark = read_file("./templates/inlined-styles-color-dark.css")
    header_template = read_file("./templates/head.html")
    header_template = header_template.replace(
        "<!-- INLINED_STYLES_LIGHT -->", 
        "<style id='inlined-styles-colors'>\r\n" + inlined_styles_light + "    </style>"
    )
    header_template = header_template.replace(
        "<!-- INLINED_STYLES_DARK -->", 
        "<style id='inlined-styles-colors-dark' media='all and (prefers-color-scheme: dark)'>\r\n" + inlined_styles_dark + "    </style>"
    )
    templates["head"] = header_template
    templates["table-of-contents"] = read_file("./templates/table-of-contents.html")
    templates["page-metadata"] = read_file("./templates/page-metadata.html")
    templates["body-noscript"] = read_file("./templates/body-noscript.html")
    templates["header-navigation"] = read_file("./templates/header-navigation.html")
    return templates

def get_root_href():
    root_href = "http://127.0.0.1:8080"
    if is_prod():
        root_href = "https://dubia.cc"
    return root_href

def load_articles():
    articles = {}
    for lang in os.listdir("./articles"):
        if lang == ".DS_Store":
            continue
        if not(os.path.isdir("./" + lang)):
            os.mkdir("./" + lang)
        for slug in os.listdir("./articles/" + lang):
            if slug == ".DS_Store":
                continue
            filepath = "./articles/" + lang + "/" + slug + "/index.md.json"
            if os.path.isfile(filepath):
                index_md = read_file(filepath)
                articles["" + lang + "/" + slug] = json.loads(index_md)
            else:
                print("WARN: missing file /articles/" + lang + "/" + slug + "/index.md.json")
                print("WARN: run md2json in the root dir to fix this issue")
    return articles

# Generates a .gitignore so that during development, the generated 
# articles aren't comitted to source control
def generate_gitignore(articles_dict):
    filenames = list(map(lambda x: x + ".html", articles_dict.keys()))
    dirs = []
    for k in articles_dict.keys():
        dirs.append(k.split("/")[0])
    filenames.extend(map(lambda x: "/" + x, dirs))
    filenames.append("/venv")
    filenames.append("*.md.json")
    filenames.append("/md2json/target")
    filenames.append("/md2json/out.txt")
    filenames.append("/venv/*")
    filenames.sort()
    filenames = list(set(filenames))
    gitignore = "\r\n".join(filenames)
    return gitignore

# returns the <head> element of the page
def head(templates, lang, title):
    head = templates["head"]
    head = head.replace("$$TITLE$$", title)
    return head

def header_navigation(templates, lang):

    homepage_desc = "Homepage: categorized list of articles index"
    logo = "/static/img/logo/logo-smooth.svg#logo"
    about_site_desc = "Site ideals, source, content, traffic, examples, license"
    about_site_title = "<span class='mobile-not'>About</span> Site"
    about_me_desc = "Who am I online, what have I done, what am I like? Contact information; sites I use; things I&#39;ve worked on"
    about_me_title = "<span class='mobile-not'>About</span> Me"
    changelog_desc = "Changelog of what’s new or updated"
    changlog_title = "New Essays"
    newest_desc = "Most recent link annotations"
    newest_title = "New Links"
    donate_desc = "Link to Patreon donation profile to support my writing"
    donate_title = "Donate"
    donate_href = ""

    header_navigation = templates["header-navigation"]
    header_navigation = header_navigation.replace("$$HOMEPAGE_DESC$$", homepage_desc)
    header_navigation = header_navigation.replace("$$LOGO$$", logo)
    header_navigation = header_navigation.replace("$$ABOUT_SITE_DESCT$$", about_site_desc)
    header_navigation = header_navigation.replace("$$ABOUT_SITE_TITLE$$", about_site_title)
    header_navigation = header_navigation.replace("$$ABOUT_ME_DESC$$", about_me_desc)
    header_navigation = header_navigation.replace("$$ABOUT_ME_TITLE$$", about_me_title)
    header_navigation = header_navigation.replace("$$CHANGELOG_DESC$$", changelog_desc)
    header_navigation = header_navigation.replace("$$CHANGELOG_TITLE$$", changlog_title)
    header_navigation = header_navigation.replace("$$ABOUT_ME_TITLE$$", about_me_title)
    header_navigation = header_navigation.replace("$$NEWEST_DESCT$$", newest_desc)
    header_navigation = header_navigation.replace("$$NEWEST_TITLE$$", newest_title)
    header_navigation = header_navigation.replace("$$DONATE_DESC$$", donate_desc)
    header_navigation = header_navigation.replace("$$DONATE_TITLE$$", donate_title)
    header_navigation = header_navigation.replace("$$DONATE_HREF$$", donate_href)
    
    return header_navigation

def link_tags(templates, lang, tags):
    link_tags = "<div class='link-tags'><p>$$TAGS$$</p></div>"
    tags_str = ""
    for t in tags:
        t_descr = "Link to " + t + " tag"
        t_data = "<a href='$$ROOT_HREF$$/" + lang + "/tag/" + t
        t_data += "' class='link-tag link-page link-annotated icon-not has-annotation spawns-popup' rel='tag' data-attribute-title='"
        t_data += t_descr + "'>" + t + "</a>"
        if t != tags[-1]:
            t_data += ", "
        tags_str += t_data
    link_tags = link_tags.replace("$$TAGS$$", tags_str)  
    return link_tags

def page_desciption(templates, lang):
    page_descr = "Hexerei ist der Gebrauch von angeblich übernatürlichen magischen Kräften."
    page_desciption = "<div class='page-description'><p>$$PAGE_DESCR$$</p></div>"
    page_desciption = page_desciption.replace("$$PAGE_DESCR$$", page_descr)
    return page_desciption

def page_metadata(templates, lang):
    
    page_metadata = templates["page-metadata"]

    date_desc = "The date range 2020-09-27–2022-11-10 lasted 2 years (775 days), ending 2 years ago."
    date_title = "2020-09-27<span class='subsup'><sup>–</sup><sub>2y</sub></span>2022-11-10"	
    backlinks_desc = "Reverse citations/backlinks for this page (the list of other pages which link to this page)."
    backlinks_title = "⁠backlinks"
    similar_desc = "Similar links for this link (by text embedding)."
    similar_title = "⁠similar"
    bibliography_desc = "Bibliography of links cited in this page (forward citations)."
    bibliography_title = "bibliography"
    
    page_metadata = page_metadata.replace("$$DATE_DESC$$", date_desc)
    page_metadata = page_metadata.replace("$$DATE_TITLE$$", date_title)
    page_metadata = page_metadata.replace("$$BACKLINKS_DESC$$", backlinks_desc)
    page_metadata = page_metadata.replace("$$BACKLINKS_TITLE$$", backlinks_title)
    page_metadata = page_metadata.replace("$$SIMILAR_DESC$$", similar_desc)
    page_metadata = page_metadata.replace("$$SIMILAR_TITLE$$", similar_title)
    page_metadata = page_metadata.replace("$$BIBLIOGRAPHY_DESC$$", bibliography_desc)
    page_metadata = page_metadata.replace("$$BIBLIOGRAPHY_TITLE$$", bibliography_title)       

    return page_metadata


def escape_html(text):
    """Escapes special characters in text for HTML."""
    import html
    return html.escape(text, quote=True)

def process_text_with_wbr(text):
    """
    Inserts <wbr> tags at every '/' or '-' in the text.
    """
    return text.replace("/", "/<wbr />").replace("-", "-<wbr />")

def render_abstract_from_items(abstract):
    """
    Converts the abstract data structure into corresponding HTML.

    Args:
        abstract (list): A nested list where each item represents a paragraph
                         with text or link elements.

    Returns:
        str: The HTML representation of the abstract.
    """
    html_output = []
    for paragraph in abstract:
        # Start a new paragraph with the appropriate class and style
        if paragraph is abstract[0]:  # First paragraph has a specific class
            html_output.append('<p class="first-block first-graf block" style="--bsm: 0;">')
        else:
            html_output.append('<p class="block" style="--bsm: 0;">')

        for element in paragraph:
            if element["ty"] == "text":
                # Process plain text, inserting <wbr> if necessary
                html_output.append(process_text_with_wbr(escape_html(element["text"])))
            elif element["ty"] == "link":
                # Process links with optional attributes
                link_attrs = [
                    f'href="{escape_html(element["href"])}"',
                    f'class="link-annotated link-page has-icon has-annotation spawns-popup"',
                ]
                if "id" in element:
                    link_attrs.append(f'id="{escape_html(element["id"])}"')
                link_attrs.append(f'data-attribute-title="{escape_html(element["desc"])}"')

                # Render the link
                html_output.append(
                    f'<a {" ".join(link_attrs)}>'
                    f'{escape_html(element["text"])}'
                    '<span class="link-icon-hook">⁠</span></a>'
                )

        # Close the paragraph
        html_output.append("</p>")

    return "\n".join(html_output)

def body_abstract(templates, lang):

    style = "class='first-block block blockquote-level-1' style='--bsm: 0;'"
    body_abstract = "<div class='abstract'><blockquote " + style + ">$$ABSTRACT_CONTENT$$</blockquote></div>"

    abstract = render_abstract_from_items([
        [
            {"ty": "text", "type": "block", "text": "Ab­stract of ar­ti­cle sum­ma­riz­ing the page. For de­sign phi­los­o­phy, see"},
            {
                "ty": "link", 
                "href": "/design", 
                "id": "gwern-design", # optional
                "desc": "&#39;Design Of This Website&#39;, Branwen 2010", 
                "text": "“De­sign Of This Web­site”"
            },
            {"ty": "text", "text": "."},
        ],
        [
            {"ty": "text", "type": "block", "text": "“Lorem Ipsum” is a test page which ex­er­cises all stan­dard func­tion­al­ity and fea­tures of Gwern.net, from stan­dard Pan­doc"},
            {
                "ty": "link", 
                "href": "https://en.wikipedia.org/wiki/Markdown", 
                "desc": "Markdown", 
                "text": "Markdown"
            },
            {"ty": "text", "text": " like block­quotes / head­ers / ta­bles / im­ages, to cus­tom fea­tures like side­notes, mar­gin notes, left / right-floated and full width im­ages, columns, epigraphs, ad­mo­ni­tions, small / wide ta­bles, small­caps, col­lapse sec­tions, link an­no­ta­tions, link icons. It par­tic­u­larly stresses tran­sclu­sion func­tion­al­ity, as it is bro­ken up into mul­ti­ple Lorem sub-pages which are tran­scluded into the mas­ter Lorem page."},
        ]
    ])

    body_abstract = body_abstract.replace("$$ABSTRACT_CONTENT$$", abstract)
    return body_abstract

def body_noscript(templates, lang):
    body_noscript = templates["body-noscript"]
    return body_noscript


# SCRIPT STARTS HERE

root_href = get_root_href()
templates = load_templates()
articles = load_articles()

for slug, readme in articles.items():

    readme_tags = ["hexe", "mittelalter", "fruehe-neuzeit", "inquisition"]

    lang = slug.split("/")[0]
    html = templates["index"]
    html = html.replace("$$SKIP_TO_MAIN_CONTENT$$", "Skip to main content")
    html = html.replace("$$TITLE$$", "Hexenverfolgung")
    html = html.replace("<!-- HEAD_TEMPLATE_HTML -->", head(templates, lang, "Hello"))
    html = html.replace("<!-- HEADER_NAVIGATION -->", header_navigation(templates, lang))
    html = html.replace("<!-- LINK_TAGS -->", link_tags(templates, lang, readme_tags))
    html = html.replace("<!-- PAGE_DESCRIPTION -->", page_desciption(templates, lang))
    html = html.replace("<!-- PAGE_METADATA -->", page_metadata(templates, lang))
    html = html.replace("<!-- BODY_ABSTRACT -->", body_abstract(templates, lang))
    html = html.replace("<!-- BODY_NOSCRIPT -->", body_noscript(templates, lang))
    html = html.replace("$$ROOT_HREF$$", root_href)
    html = html.replace("$$PAGE_HREF$$", root_href + "/" + slug)

    write_file(html, "./" + slug + ".html")

write_file(generate_gitignore(articles), "./.gitignore")

# special pages: /de - list all articles
# /de/neu - list newest articles
# /index.html - search page (automatic language)