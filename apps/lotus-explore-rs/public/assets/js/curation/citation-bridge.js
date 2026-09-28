// Scholia bridge for the curation page.
//
// Wrapped in an IIFE on purpose. A classic <script> shares the global lexical
// scope, so a second copy of this file used to throw
// "Identifier 'CITATION_BASE_PATH' has already been declared" — a parse-time
// error, before any of the code below runs. The curation page does get two
// copies injected (measured: 4 bridge <script> tags on a direct load of
// /curation, 2 SyntaxErrors), and the redeclaration cost the page its
// best-practices score.
//
// The single-flight load cache lives on `window.__lotusCitation` rather than in
// a module-local `let`, so a second copy of this file reuses the in-flight load
// instead of starting a competing one and orphaning the first.
(function () {
    "use strict";

    const CITATION_BASE_PATH = (() => {
        const base = document.documentElement.getAttribute("data-lotus-base-path") || "/";
        return base.endsWith("/") ? base : `${base}/`;
    })();
    const CITATION_JS_SRC = `${CITATION_BASE_PATH}assets/vendor/citation-js/citation.js`;
    const bridge = (window.__lotusCitation = window.__lotusCitation || {});

    function loadCitationJs() {
        if (typeof globalThis.__Cite === "function") {
            return Promise.resolve(globalThis.__Cite);
        }
        if (bridge.loadPromise) {
            return bridge.loadPromise;
        }

        bridge.loadPromise = new Promise((resolve, reject) => {
            const existing = document.querySelector(`script[src="${CITATION_JS_SRC}"]`);
            const complete = () => {
                try {
                    let CiteCtor = null;
                    if (typeof globalThis.Cite === "function") {
                        CiteCtor = globalThis.Cite;
                    } else if (typeof require === "function") {
                        CiteCtor = require("citation-js").Cite;
                    }

                    if (typeof CiteCtor === "function") {
                        globalThis.__Cite = CiteCtor;
                        resolve(CiteCtor);
                    } else {
                        reject(new Error("citation.js loaded but did not expose a Cite constructor"));
                    }
                } catch (_error) {
                    reject(new Error("Scholia citation.js bundle failed to expose Cite"));
                }
            };

            if (existing) {
                existing.addEventListener("load", complete, { once: true });
                existing.addEventListener(
                    "error",
                    () => reject(new Error("citation.js failed to load")),
                    { once: true }
                );
                return;
            }

            const script = document.createElement("script");
            script.src = CITATION_JS_SRC;
            script.async = true;
            script.crossOrigin = "anonymous";
            script.addEventListener("load", complete, { once: true });
            script.addEventListener(
                "error",
                () => reject(new Error("citation.js failed to load")),
                { once: true }
            );
            document.head.appendChild(script);
        }).catch((error) => {
            bridge.loadPromise = null;
            throw error;
        });

        return bridge.loadPromise;
    }

    async function resolveCitationBridge() {
        if (typeof globalThis.__Cite === "function") {
            return globalThis.__Cite;
        }
        return await loadCitationJs();
    }

    async function fetchDoiCslJson(doi) {
        const response = await fetch(`https://doi.org/${encodeURIComponent(doi)}`, {
            headers: {
                Accept: "application/vnd.citationstyles.csl+json, application/json;q=0.9",
            },
        });
        if (!response.ok) {
            throw new Error(`DOI metadata fetch failed: HTTP ${response.status}`);
        }
        return await response.json();
    }

    bridge.quickStatements = async function quickStatements(doi) {
        const trimmed = String(doi || "").trim();
        if (!trimmed) {
            return "";
        }
        const CiteCtor = await resolveCitationBridge();
        const cslJson = await fetchDoiCslJson(trimmed);
        let cite;
        try {
            cite = new CiteCtor(cslJson);
        } catch (_ctorError) {
            cite = await CiteCtor.async(cslJson);
        }
        const output = cite.format("quickstatements");
        return typeof output === "string" ? output : "";
    };
})();
