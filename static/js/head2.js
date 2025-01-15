var expand = document.getElementById('expand-controls');
var collapse = document.getElementById('collapse-controls');
var page_toolbar_container = document.getElementById('page-toolbar');
var widgets = document.getElementById('page-toolbar-widgets');

function toggleToolbar() {
    if (!page_toolbar_container) {
        return;
    }

    if (page_toolbar_container.classList.contains("collapsed")) {
        page_toolbar_container.classList.remove("collapsed");
    } else {
        page_toolbar_container.classList.add("collapsed");
    }
}

if (expand) {
    expand.onmouseup = toggleToolbar;
}