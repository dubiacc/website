Extracts.addTargetsWithin(GW.floatingHeader.linkChain);
Extracts.config.hooklessLinksContainersSelector += `, #${GW.search.searchWidgetId}`;
Extracts.addTargetsWithin(GW.search.searchWidget);

// 7446

//  Configure pop-frame behavior.
if (Extracts.popFrameProvider == Popups) {
    //  Configure popup positioning and click response.
    GW.search.searchWidgetLink.preferPopupSidePositioning = () => true;
    GW.search.searchWidgetLink.cancelPopupOnClick = () => false;
    GW.search.searchWidgetLink.keepPopupAttachedOnPin = () => true;

    //  Pin popup and focus search box if widget is clicked.
    GW.search.searchWidgetLink.addActivateEvent((event) => {
        GW.search.pinSearchPopup();
    });

    //  Add popup spawn event handler.
    GW.notificationCenter.addHandlerForEvent("Popups.popupDidSpawn", popFrameSpawnEventHandler, {
        condition: (info) => (info.popup.spawningTarget == GW.search.searchWidgetLink)
    });
    //	Add popup despawn event handler.
    GW.notificationCenter.addHandlerForEvent("Popups.popupWillDespawn", popFrameDespawnEventHandler, {
        condition: (info) => (info.popup.spawningTarget == GW.search.searchWidgetLink)
    });
} else {
    //  Add popin inject event handler.
    GW.notificationCenter.addHandlerForEvent("Popins.popinDidInject", popFrameSpawnEventHandler, {
        condition: (info) => (info.popin.spawningTarget == GW.search.searchWidgetLink)
    });
}

doWhenPageLoaded(() => {
    if (Extracts.popFrameProvider == Popups)
        document.addEventListener("keyup", GW.keyCommands.keyUp);
});