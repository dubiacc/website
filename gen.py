import sys, base64, os

def is_prod():
    l = len(sys.argv)
    if l > 1:
        return sys.argv[1] == "--production"
    else:
        return False
    
def read_file(path):
    text_file = open(path, 'r')
    text_file_contents = text_file.read()
    text_file.close()
    return text_file_contents

def read_file_base64(path):
    encoded_string = ""
    with open(path, "rb") as image_file:
        encoded_string = base64.b64encode(image_file.read()).decode()
    return encoded_string

def write_file(string, path):
    text_file = open(path, "w+", newline='')
    text_file.write(string)
    text_file.close()

def chunks(lst, n):
    """Yield successive n-sized chunks from lst."""
    for i in range(0, len(lst), n):
        yield lst[i:i + n]

articles = {}
for (dirpath, dirnames, filenames) in os.walk("./articles"):
    for lang in dirnames:
        for (dirpath, slugs, filenames) in os.walk("./articles/" + lang):
            for slug in slugs:
                index_md = read_file("./articles/" + lang + "/" + slug + "/index.md")
                articles[slug] = index_md.splitlines()

filenames = list(map(lambda x: x + ".html", articles.keys()))
gitignore = "\r\n".join(filenames)
write_file(gitignore, "./.gitignore")

index_html = read_file("./skeleton.html")
root_href = "http://127.0.0.1:8080"
if is_prod():
    root_href = "https://dubia.cc"

inlined_styles_light = read_file("./templates/inlined-styles.css")
inlined_styles_dark = read_file("./templates/inlined-styles-color-dark.css")
header_template = read_file("./templates/head.html")
header_template = header_template.replace(
    "<!-- INLINED_STYLES_LIGHT -->", 
    "<style id='inlined-styles-colors'>\r\n" + inlined_styles_light + "    </style>"
)
header_template = header_template.replace(
    "<!-- INLINED_STYLES_DARK -->", 
    "<style id='inlined-styles-colors-dark' media='not all'>\r\n" + inlined_styles_dark + "    </style>"
)

for slug, readme in articles.items():
    page_href = root_href + "/" + slug
    header = header_template.replace("$$TITLE$$", slug)
    html = index_html.replace("<!-- HEAD_TEMPLATE_HTML -->", header)
    html = html.replace("$$ROOT_HREF$$", root_href)
    html = html.replace("$$PAGE_HREF$$", page_href)
    write_file(html, "./" + slug + ".html")

# <link id="inlined-styles" rel="stylesheet" href="./inlined-styles.css">
# <link id="inlined-styles-colors-dark" rel="stylesheet" href="./inlined-styles-color-dark.css" media="not all">
  