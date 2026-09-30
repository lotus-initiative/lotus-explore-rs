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

    // The loader script is *not* fetched from here. `document_head::CurationScripts`
    // adds the `<script>` tag itself, from a path Rust got back from the bundler.
    // This file used to build the URL itself, and a path assembled at runtime is
    // invisible to `dx`: it embeds only the assets Rust names, so the file was
    // simply not in a desktop bundle and the request 404ed.
    //
    // The wasm module is the one URL that cannot be a `src`, because the loader
    // looks it up by name relative to itself and the bundled copy is
    // content-addressed. Rust publishes it on a global for `locateFile` below.
    const RDKIT_WASM_GLOBAL = "__lotusRDKitWasm";
    const bridge = (window.__lotusRdkit = window.__lotusRdkit || {});

    // The loader script is already in the document, so this only has to wait
    // for it to finish defining `initRDKitModule`. Waiting for a global is the
    // only option left now that this file does not create the tag: a classic
    // `<script src>` tag with `defer` runs in document order, and the loader is
    // tagged after this file, so by the time anything calls in, it is either
    // there or the load genuinely failed.
    function waitForInitRDKitModule(timeoutMs = 20000) {
        const start = Date.now();
        return new Promise((resolve, reject) => {
            function poll() {
                if (typeof initRDKitModule === "function") {
                    resolve(initRDKitModule);
                    return;
                }
                if (Date.now() - start >= timeoutMs) {
                    reject(
                        new Error(
                            "RDKit_minimal.js did not define initRDKitModule; " +
                                "check that it reached the bundle"
                        )
                    );
                    return;
                }
                setTimeout(poll, 16);
            }
            poll();
        });
    }

    function ensureRdkitReady() {
        if (bridge.rdkitReadyPromise) {
            return bridge.rdkitReadyPromise;
        }

        bridge.rdkitReadyPromise = (async () => {
            const init = await waitForInitRDKitModule();
            // `locateFile` is how the wasm module is found. Left to itself the
            // loader asks for `RDKit_minimal.wasm` next to the script, which
            // only works if the file kept that exact name. The bundled copy is
            // content-addressed, so it does not.
            const wasm = window[RDKIT_WASM_GLOBAL];
            const RDKit = wasm
                ? await init({ locateFile: () => wasm })
                : await init();

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

            bridge.rdkit = {
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
            return bridge.rdkit;
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
