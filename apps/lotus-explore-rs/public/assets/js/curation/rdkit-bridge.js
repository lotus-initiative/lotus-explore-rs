// RDKit bridge for the curation page.
//
// Wrapped in an IIFE on purpose. A classic <script> shares the global lexical
// scope, so a second copy of this file used to throw
// "Identifier 'RDKIT_BASE_PATH' has already been declared" — a parse-time
// error, before any of the code below runs. The curation page does get two
// copies injected (measured: 4 bridge <script> tags on a direct load of
// /curation, 2 SyntaxErrors), and the redeclaration cost the page its
// best-practices score.
//
// The single-flight caches live on `window.__lotusRdkit` rather than in
// module-local `let`s, so a second copy of this file reuses the in-flight load
// instead of starting a competing one and orphaning the first.
(function () {
    "use strict";

    const RDKIT_BASE_PATH = (() => {
        const base = document.documentElement.getAttribute("data-lotus-base-path") || "/";
        return base.endsWith("/") ? base : `${base}/`;
    })();
    const RDKIT_JS_SRC = `${RDKIT_BASE_PATH}assets/vendor/rdkit/RDKit_minimal.js`;
    const bridge = (window.__lotusRdkit = window.__lotusRdkit || {});

    function waitForInitRDKitModule(timeoutMs = 12000) {
        const start = Date.now();
        return new Promise((resolve, reject) => {
            function poll() {
                if (typeof initRDKitModule === "function") {
                    resolve(initRDKitModule);
                    return;
                }
                if (Date.now() - start >= timeoutMs) {
                    reject(new Error("RDKit_minimal.js timed out loading"));
                    return;
                }
                setTimeout(poll, 16);
            }
            poll();
        });
    }

    function loadRdkitJs() {
        if (typeof initRDKitModule === "function") {
            return Promise.resolve(initRDKitModule);
        }
        if (bridge.rdkitScriptLoadPromise) {
            return bridge.rdkitScriptLoadPromise;
        }

        bridge.rdkitScriptLoadPromise = new Promise((resolve, reject) => {
            const existing = document.querySelector(`script[src="${RDKIT_JS_SRC}"]`);
            const complete = () => {
                waitForInitRDKitModule().then(resolve, reject);
            };

            if (existing) {
                complete();
                return;
            }

            const script = document.createElement("script");
            script.src = RDKIT_JS_SRC;
            script.async = true;
            script.crossOrigin = "anonymous";
            script.addEventListener("load", complete, { once: true });
            script.addEventListener(
                "error",
                () => reject(new Error("RDKit_minimal.js failed to load")),
                { once: true }
            );
            document.head.appendChild(script);
        }).catch((error) => {
            bridge.rdkitScriptLoadPromise = null;
            throw error;
        });

        return bridge.rdkitScriptLoadPromise;
    }

    function ensureRdkitReady() {
        if (bridge.rdkitReadyPromise) {
            return bridge.rdkitReadyPromise;
        }

        bridge.rdkitReadyPromise = (async () => {
            const init = await loadRdkitJs();
            const RDKit = await init();

            function withMol(smiles, callback) {
                const trimmed = String(smiles || "").trim();
                if (!trimmed) {
                    throw new Error("smiles is required");
                }
                const mol = RDKit.get_mol(trimmed);
                if (!mol) {
                    throw new Error("rdkit.js could not parse the structure");
                }
                try {
                    return callback(mol);
                } finally {
                    if (mol && typeof mol.delete === "function") {
                        mol.delete();
                    }
                }
            }

            function descriptorExactMass(descriptors) {
                const keys = [
                    "exact_molecular_weight",
                    "ExactMolWt",
                    "exactmw",
                    "exact_mw",
                ];
                for (const key of keys) {
                    const value = descriptors?.[key];
                    if (typeof value === "number" && Number.isFinite(value)) {
                        return value;
                    }
                }
                throw new Error("rdkit.js descriptors did not include exact mass");
            }

            function stripStereoFromSmiles(smiles) {
                return smiles.replace(/@{1,2}/g, "").replace(/[/\\]/g, "");
            }

            return {
                convert(smiles) {
                    return withMol(smiles, (mol) => {
                        const isomericsmiles = mol.get_smiles(
                            JSON.stringify({ canonical: true, isomericSmiles: true })
                        );
                        let canonicalsmiles = mol.get_smiles(
                            JSON.stringify({ canonical: true, isomericSmiles: false })
                        );
                        if (
                            canonicalsmiles.includes("@") ||
                            canonicalsmiles.includes("/") ||
                            canonicalsmiles.includes("\\")
                        ) {
                            canonicalsmiles = stripStereoFromSmiles(canonicalsmiles);
                        }
                        const inchi = mol.get_inchi();
                        if (!inchi) {
                            throw new Error("rdkit.js could not generate InChI");
                        }
                        const inchikey = RDKit.get_inchikey_for_inchi(inchi);
                        if (!inchikey) {
                            throw new Error("rdkit.js could not generate InChIKey");
                        }
                        return { canonicalsmiles, isomericsmiles, inchi, inchikey };
                    });
                },
                exactMass(smiles) {
                    return withMol(smiles, (mol) => {
                        const descriptors = JSON.parse(mol.get_descriptors());
                        return descriptorExactMass(descriptors);
                    });
                },
                hasUndefinedStereo(smiles) {
                    return withMol(smiles, (mol) => {
                        const tags = JSON.parse(mol.get_stereo_tags() || "[]");
                        if (!Array.isArray(tags)) {
                            return false;
                        }
                        return tags.some((tag) => {
                            const text = String(tag || "").toLowerCase();
                            return text.includes("unspecified") || text.includes("unknown");
                        });
                    });
                },
            };
        })().catch((error) => {
            bridge.rdkitReadyPromise = null;
            throw error;
        });

        return bridge.rdkitReadyPromise;
    }

    bridge.ready = function ready() {
        return ensureRdkitReady();
    };

    bridge.convert = async function convert(smiles) {
        const rdkit = await ensureRdkitReady();
        return rdkit.convert(smiles);
    };

    bridge.exactMass = async function exactMass(smiles) {
        const rdkit = await ensureRdkitReady();
        return rdkit.exactMass(smiles);
    };

    bridge.hasUndefinedStereo = async function hasUndefinedStereo(smiles) {
        const rdkit = await ensureRdkitReady();
        return rdkit.hasUndefinedStereo(smiles);
    };
})();
