const TOREPLACE = [
    "#", "*", "\n", "?", "!", ">", "<",
    "https://", "k-bibel.de", ":", "/", "\\","\r", "\"", 
    "-", "_", "[", "]", "(", ")", ",", ".", 
];

function doAjax(options) {
    options = Object.assign({
        location: document.location,
        method: "GET",
        params: null,
        serialization: "URL",
        responseType: null,
        headers: null,
        onSuccess: null,
        onFailure: null
    }, options);

    let req = new XMLHttpRequest();

    req.addEventListener("load", (event) => {
        if (event.target.status < 400) {
            options.onSuccess?.(event);
        } else {
            options.onFailure?.(event);
        }
    });
    req.addEventListener("error", (event) => {
        options.onFailure?.(event);
    });

    let location = options.location
        + ((options.params != null
            && options.method == "GET")
            ? "?" + urlEncodeQuery(options.params)
            : "");
    req.open(options.method, location);

    if (options.responseType)
        req.responseType = options.responseType;

    if (options.headers)
        for (let [headerName, headerValue] of Object.entries(options.headers))
            req.setRequestHeader(headerName, headerValue);

    if (options.method == "POST") {
        let payload;
        switch (options.serialization) {
            case "JSON":
                payload = JSON.stringify(options.params);
                req.setRequestHeader("Content-Type", "application/json");
                break;
            case "URL":
            default:
                payload = urlEncodeQuery(options.params);
                req.setRequestHeader("Content-Type", "application/x-www-form-urlencoded");
                break;
        }

        req.send(payload);
    } else {
        req.send();
    }
}

function goToFirstArticle(e) {
    var searchterm = document.getElementById("index-search-input").value;
    if (searchterm.length == 0) {
        return false;
    } else if (searchterm.length < 3) {
        return false;
    }
    
    var results = searchArticlesLocal(searchterm);
    if (results.length < 1) {
        return false;
    }
    
    const result = results[0];
    let url;
    
    if (result.doc_type === "document") {
        const parts = result.id.split('/');
        const slug = parts[parts.length - 1];
        url = "/$$LANG$$/docs/" + result.author + "/" + slug;
    } else {
        url = "/$$LANG$$/" + result.id;
    }
    
    window.location.href = url;
    return false;
}

function searchAndDisplayArticles(e) {
    var searchterm = document.getElementById("index-search-input").value;
    var target = document.getElementById("index-search-results");
    var no_results = "<p id='no-results' style='padding-left:10px;'>$$NO_RESULTS$$</p>"
    
    if (searchterm.length == 0) {
        target.innerHTML = "";
        return false;
    } else if (searchterm.length < 3) {
        target.innerHTML = no_results;
        return false;
    }
    
    var results = searchArticlesLocal(searchterm);
    
    if (results.length < 1) {
        target.innerHTML = no_results;
        return false;
    } else {
        let a = "";
        for (let i = 0; i < results.length; i++) {
            const result = results[i];
            let id = result.id;
            let title = result.title;
            let url;
            
            // Generate correct URL based on result type
            if (result.doc_type === "document") {
                const parts = id.split('/');
                const slug = parts[parts.length - 1];
                url = "/$$LANG$$/docs/" + result.author + "/" + slug;
            } else {
                url = "/$$LANG$$/" + id;
            }
            
            a += "<li class='link-modified-recently-list-item dark-mode-invert'>";
            a += "<p class='in-list first-graf block' style='--bsm: 0;'>";
            a += "<a href='" + url + "' id='sr-" + id;
            a += "' class='link-annotated link-page in-list has-annotation spawns-popup' ";
            a += " data-attribute-title='" + title + "'>" + title + "</a>";
            a += "</p>";
            a += "</li>";
        }
        target.innerHTML = "<ul class='list'>" + a + "</ul>";
        return false;
    }
}

function searchArticlesLocal(searchterm) {
    if (searchterm.length == 0) {
        return [];
    }

    if (!(window.articles && window.articles != null && window.articles != undefined)) {
        console.error("searchArticlesLocal: window.articles not yet initialized");
        return [];
    }

    var searchterm = searchterm.toLowerCase();
    for (q of TOREPLACE) {
        searchterm = searchterm.replaceAll(q, "");
    }
    
    var results = [];
    var results_contains = {};
    
    // First search article/document slugs
    for (var id in window.articles.articles) {
        if (results.length > 3) {
            break;
        }
        if (results_contains.hasOwnProperty(id)) {
            continue;
        }
        
        const item = window.articles.articles[id];
        
        if (id.toLowerCase().includes(searchterm)) {
            if (item.doc_type === "document") {
                results.push({
                    id: id, 
                    title: item.title,
                    doc_type: "document",
                    author: item.author
                });
            } else {
                results.push({
                    id: id, 
                    title: item.title
                });
            }
            results_contains[id] = "";
            continue;
        }
    }

    // Then search article/document titles
    for (var id in window.articles.articles) {
        if (results.length > 3) {
            break;
        }
        if (results_contains.hasOwnProperty(id)) {
            continue;
        }
        
        const item = window.articles.articles[id];
        var title = item.title;
        
        if (title.toLowerCase().includes(searchterm)) {
            if (item.doc_type === "document") {
                results.push({
                    id: id, 
                    title: item.title,
                    doc_type: "document",
                    author: item.author
                });
            } else {
                results.push({
                    id: id, 
                    title: item.title
                });
            }
            results_contains[id] = "";
            continue;
        }
    }

    // Finally search content
    for (var id in window.articles.articles) {
        if (results.length > 3) {
            break;
        }
        if (results_contains.hasOwnProperty(id)) {
            continue;
        }
        
        const item = window.articles.articles[id];
        var sha256 = item.sha256;
        var mdfile = localStorage.getItem("b" + sha256);
        
        if (!mdfile) {
            // If document content not in localStorage, try to load it
            if (item.doc_type === "document") {
                loadDocumentContent(id, sha256);
            }
            continue;
        }
        
        if (mdfile.includes(searchterm)) {
            if (item.doc_type === "document") {
                results.push({
                    id: id, 
                    title: item.title,
                    doc_type: "document",
                    author: item.author
                });
            } else {
                results.push({
                    id: id, 
                    title: item.title
                });
            }
            results_contains[id] = "";
        }
    }

    return results;
}

// Load document content when needed
function loadDocumentContent(id, sha256) {
    const item = window.articles.articles[id];
    if (!item || item.doc_type !== "document") {
        return;
    }
    
    const parts = id.split('/');
    const slug = parts[parts.length - 1];
    const author = item.author;
    
    console.log("downloading document content for $$LANG$$/docs/" + author + "/" + slug);
    doAjax({
        location: `${location.origin}/docs/$$LANG$$/${author}/${slug}.md`,
        onSuccess: function(event) {
            let t = event.target.responseText.toLowerCase();
            for (q of TOREPLACE) {
                t = t.replaceAll(q, "");
            }
            localStorage.setItem("b" + sha256, t);
        }
    });
}

function checkArticlesAreInitialized(force) {
    var do_force = force === true;

    if (!(window.articles && window.articles != null && window.articles != undefined)) {
        console.error("checkArticlesAreInitialized called but window.articles not initialized!");
        return;
    }

    for (var id in window.articles.articles) {
        const item = window.articles.articles[id];
        let sha256 = item.sha256;
        let cachedarticle = localStorage.getItem("b" + sha256);
        
        if (!do_force && cachedarticle) {
            continue;
        }
        
        // For documents, use a different path scheme
        if (item.doc_type === "document") {
            const parts = id.split('/');
            const slug = parts[parts.length - 1];
            const author = item.author;
            
            console.log("downloading document for $$LANG$$/docs/" + author + "/" + slug);
            doAjax({
                location: `${location.origin}/docs/$$LANG$$/${author}/${slug}.md`,
                onSuccess: function(event) {
                    let t = event.target.responseText.toLowerCase();
                    for (q of TOREPLACE) {
                        t = t.replaceAll(q, "");
                    }
                    localStorage.setItem("b" + sha256, t);
                }
            });
        } else {
            // Regular article
            console.log("downloading index.md file for $$LANG$$/" + id);
            doAjax({
                location: `${location.origin}/articles/$$LANG$$/${id}/index.md`,
                onSuccess: function(event) {
                    let t = event.target.responseText.toLowerCase();
                    for (q of TOREPLACE) {
                        t = t.replaceAll(q, "");
                    }
                    localStorage.setItem("b" + sha256, t);
                }
            });
        }
    }
}

window.searchArticlesLocal = searchArticlesLocal;
window.checkArticlesAreInitialized = checkArticlesAreInitialized;

function initSearchIndex(force) {
    var do_force = force === true;
    var version = "$$VERSION$$";
    var a = localStorage.getItem('articles');

    if (!do_force) {
        if (
            window.articles && 
            window.articles != null && 
            window.articles != undefined && 
            window.articles.git == version) {
            checkArticlesAreInitialized(force);
            return;
        } else {
            try {
                var b = JSON.parse(a);
                if (
                    b && 
                    b != null && 
                    b != undefined && 
                    b.git == version) {
                    window.articles = b;
                    checkArticlesAreInitialized(force);
                    return;
                }
            } catch(e) {
                // JSON parsing failed, download new index
            }
        }
    }

    console.log("initSearchIndex: downloading /$$LANG$$/index.json");

    // Download the unified index containing both articles and documents
    doAjax({
        location: `${location.origin}/$$LANG$$/index.json`,
        onSuccess: function(event) {
            var tar = JSON.parse(event.target.responseText);
            localStorage.setItem('articles', JSON.stringify(tar));
            window.articles = tar;
            checkArticlesAreInitialized(force);
        },
        onError: function(event) {
            console.error(event);
        }
    });
}

window.initSearchIndex = initSearchIndex;

initSearchIndex(false); 
document.getElementById("index-search-form").onsubmit = function(event) { goToFirstArticle(event); return false; };
document.getElementById("index-search-input").onkeyup = function(event) { searchAndDisplayArticles(event); return false; };
document.getElementById("index-search-button").onclick = function(event) { goToFirstArticle(event); return false; };