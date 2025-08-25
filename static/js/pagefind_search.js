(async () => {
    const pagefind = await import("/static/pagefind/pagefind.js");

    const search = async (term) => {
        let results = [];
        let options = {};

        if (term.startsWith("doc:")) {
            term = term.substring(4);
            options.filters = { type: "document" };
        } else {
            options.sort = { priority: "asc" };
        }

        const search = await pagefind.search(term, options);
        results = await Promise.all(search.results.map(r => r.data()));

        return results;
    };

    const searchInput = document.getElementById("index-search-input");
    const searchResults = document.getElementById("index-search-results");

    const displayResults = (results) => {
        searchResults.innerHTML = "";
        if (results.length === 0) {
            searchResults.innerHTML = "<p>No results found.</p>";
            return;
        }

        for (const result of results) {
            const resultItem = document.createElement("div");
            resultItem.className = "result-item";

            const titleDiv = document.createElement("div");
            titleDiv.className = "result-title";
            const titleLink = document.createElement("a");
            titleLink.href = result.url;
            titleLink.innerHTML = result.meta.title;
            titleDiv.appendChild(titleLink);
            resultItem.appendChild(titleDiv);

            const contextDiv = document.createElement("div");
            contextDiv.className = "result-context";

            if (result.sub_results && result.sub_results.length > 1) {
                for (const subResult of result.sub_results) {
                    const subResultDiv = document.createElement("div");
                    const subResultLink = document.createElement("a");
                    subResultLink.href = subResult.url;
                    subResultLink.innerHTML = subResult.title;
                    const excerpt = document.createElement("p");
                    excerpt.innerHTML = subResult.excerpt;

                    subResultDiv.appendChild(subResultLink);
                    subResultDiv.appendChild(excerpt);
                    contextDiv.appendChild(subResultDiv);
                }
            } else {
                const excerpt = document.createElement("p");
                excerpt.innerHTML = result.excerpt;
                contextDiv.appendChild(excerpt);
            }
            resultItem.appendChild(contextDiv);
            searchResults.appendChild(resultItem);
        }
    };

    searchInput.addEventListener("input", async (e) => {
        const term = e.target.value;
        if (term.length < 3) {
            searchResults.innerHTML = "";
            return;
        }
        const results = await search(term);
        displayResults(results);
    });
})();
