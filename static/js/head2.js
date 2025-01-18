var expand = document.getElementById('expand-controls');
var collapse = document.getElementById('collapse-controls');
var toolbar = document.getElementById('page-toolbar');

var selectCssModeAutoBtn = document.getElementById('select-css-mode-auto');
var selectCssModeLightBtn = document.getElementById('select-css-mode-light');
var selectCssModeDarkBtn = document.getElementById('select-css-mode-dark');

var disableLinkPopupsBtn = document.getElementById('disable-link-popups');
var enableLinkPopupsBtn = document.getElementById('enable-link-popups');

var enableReaderModeBtn = document.getElementById('enable-reader-mode');
var disableReaderModeBtn = document.getElementById('disable-reader-mode');
var autoReaderModeBtn = document.getElementById('auto-reader-mode');

function reloadCss() {
    let selectedMode = localStorage.getItem("dark-mode-setting") || "auto";
    let switchedElementsSelector = [
        "#inlined-styles-colors-dark",
        "#favicon-dark",
        "#favicon-apple-touch-dark"
    ].join(", ");

    let mediaAttributeValues = {
        "auto": "all and (prefers-color-scheme: dark)",
        "dark": "all",
        "light": "not all"
    };

    document.querySelectorAll(switchedElementsSelector).forEach(element => { 
        element.media = mediaAttributeValues[selectedMode]; 
    });
}

function toggleToolbar() {
    if (!toolbar) {
        return;
    }

    if (toolbar.classList.contains("collapsed")) {
        toolbar.classList.remove("collapsed");
    } else {
        toolbar.classList.add("collapsed");
    }
}

function saveCssMode(newMode) { localStorage.setItem("dark-mode-setting", newMode); }
function savePopupMode(newMode) { localStorage.setItem("extract-popins-disabled", newMode); }
function saveReaderMode(newMode) { localStorage.setItem("reader-mode-setting", newMode); }

function toggleCssModeAuto() { 
    saveCssMode("auto"); 
    reloadCss(); 
}

function toggleCssModeLight() { 
    saveCssMode("light"); 
    reloadCss(); 
}

function toggleCssModeDark() { 
    saveCssMode("dark"); 
    reloadCss(); 
}

function disablePopupLinks() { 
    savePopupMode("disabled"); 
}

function enablePopupLinks() { 
    savePopupMode("enabled"); 
}

function disableReaderMode() { 
    saveReaderMode("disabled"); 
    document.body.classList.remove("reader-mode-active");
}

function enableReaderMode() { 
    saveReaderMode("enabled");
    document.body.classList.add("reader-mode-active");
}

if (expand) { expand.onmouseup = toggleToolbar; }

if (selectCssModeAutoBtn) { selectCssModeAutoBtn.onmouseup = toggleCssModeAuto; }
if (selectCssModeLightBtn) { selectCssModeLightBtn.onmouseup = toggleCssModeLight; }
if (selectCssModeDarkBtn) { selectCssModeDarkBtn.onmouseup = toggleCssModeDark; }

if (disableLinkPopupsBtn) { disableLinkPopupsBtn.onmouseup = disablePopupLinks; }
if (enableLinkPopupsBtn) { enableLinkPopupsBtn.onmouseup = enablePopupLinks; }

if (autoReaderModeBtn) { autoReaderModeBtn.onmouseup = disableReaderMode; }
if (disableReaderModeBtn) { disableReaderModeBtn.onmouseup = disableReaderMode; }
if (enableReaderModeBtn) { enableReaderModeBtn.onmouseup = enableReaderMode; }
