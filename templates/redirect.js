<script>
    "use strict";
    var userLang = window.navigator.language || window.navigator.userLanguage;
    if (userLang.includes("de")) {
        window.location.href = '/de'
    } else {
        window.location.href = '/en'
    }
</script>
