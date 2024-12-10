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
    let id = results[0]["id"];
    window.location.href = "/en/" + id;
    return false;
}

function searchAndDisplayArticles(e) {
    var searchterm = document.getElementById("index-search-input").value;
    var target = document.getElementById("index-search-results");
    var no_results = "<p id='no-results' style='padding-left:10px;'>No results found.</p>"
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
            const element = results[i];
            let id = element["id"];
            let title = element["title"];
            a += "<li class='link-modified-recently-list-item dark-mode-invert'>";
            a += "<p class='in-list first-graf block' style='--bsm: 0;''>";
            a += "<a href='/en/" +id + "' id='sr-" + id;
            a += "' class='link-annotated link-page in-list has-annotation spawns-popup' "
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
    for (var id in window.articles.articles) {
        if (results.length > 3) {
            break;
        }
        if (results_contains.hasOwnProperty(id)) {
            continue;
        }
        if (id.toLowerCase().includes(searchterm)) {
            results.push({ "id": id, "title": window.articles.articles[id].title });
            results_contains[id] = "";
            continue;
        }
    }

    for (var id in window.articles.articles) {
        if (results.length > 3) {
            break;
        }
        if (results_contains.hasOwnProperty(id)) {
            continue;
        }
        var title = window.articles.articles[id].title;
        if (title.toLowerCase().includes(searchterm)) {
            results.push({ id: id, title: window.articles.articles[id].title });
            results_contains[id] = "";
            continue;
        }
    }

    for (var id in window.articles.articles) {
        if (results.length > 3) {
            break;
        }
        if (results_contains.hasOwnProperty(id)) {
            continue;
        }
        var sha256 = window.articles.articles[id].sha256;
        var mdfile = localStorage.getItem("b" + sha256);
        if (!mdfile) {
            continue;
        }
        if (mdfile.includes(searchterm)) {
            results.push({ id: id, title: window.articles.articles[id].title });
            results_contains[id] = "";
        }
    }

    return results;
}
window.searchArticlesLocal = searchArticlesLocal;

function checkArticlesAreInitialized(force) {

    var do_force = false;
    if (force && force === true) {
        do_force = true;
    }

    // window.articles are the newest version
    //
    // localStorage.searchindex-articles = {
    //   "git": "GIT_HASH_AT_TIME_OF_GENERATION", 
    //   "articles": { 
    //     "blah-id-of-article": {
    //       "sha256": "SHA256OFARTICLE",
    //       "title": "Blah Title of Article"
    //     }
    //   }
    // }
    // 
    // localStorage[SHA256OFARTICLE] = index.md

    if (!(window.articles && window.articles != null && window.articles != undefined)) {
        console.error("checkArticlesAreInitialized called but window.articles not initialized!");
        return;
    }

    for (var id in window.articles.articles) {

        let sha256 = window.articles.articles[id].sha256;
        let cachedarticle = localStorage.getItem("b" + sha256);
        if (!do_force) {
            if (cachedarticle) {
                continue;
            } else {
                console.log("cached article not found for sha256 " + sha256);
            }
        }

        console.log("downloading index.md file for en/" + id);
        doAjax({
            location: `${location.origin}/articles/en/${id}/index.md`,
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
window.checkArticlesAreInitialized = checkArticlesAreInitialized;

function initSearchIndex(force) {

    var do_force = false;
    if (force && force === true) {
        do_force = true;
    }

    var version = "e42cc995901bb859bae10b1b4d9de1e2260b46ee";
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
        }
    }

    console.log("initSearchIndex: downloading /en/index.json");

    // TODO: diff and delete outdated articles
    doAjax({
        location: `${location.origin}/en/index.json`,
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
