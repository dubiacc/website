<script type="text/javascript">

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

    doAjax({
        location: "/de.html",
        onSuccess: (event) => {
            var userLang = window.navigator.language || window.navigator.userLanguage;
            if (userLang.includes("de")) {
                if (document.readyState === "interactive") {
                    document.documentElement.innerHTML = event.target.response;
                } else {
                    document.addEventListener("DOMContentLoaded", function(e) {
                        document.documentElement.innerHTML = event.target.response;
                    });
                }
            }
        }
    });
</script>
