(function () {
  // 1. Non-blocking Analytics Initialization
  var loadAnalytics = function () {
    if (document.querySelector('script[src*="simpleanalyticscdn.com"]')) return;
    var s = document.createElement("script");
    s.async = true;
    s.defer = true;
    s.src = "https://scripts.simpleanalyticscdn.com/latest.js";
    document.head.appendChild(s);
  };

  var schedule = function () {
    if ("requestIdleCallback" in window) {
      requestIdleCallback(loadAnalytics);
    } else {
      setTimeout(loadAnalytics, 3000);
    }
  };
  var scheduleAfterLoad = function () {
    setTimeout(schedule, 5000);
  };
  if (document.readyState === "complete") {
    scheduleAfterLoad();
  } else {
    window.addEventListener("load", scheduleAfterLoad, { once: true });
  }
})();
