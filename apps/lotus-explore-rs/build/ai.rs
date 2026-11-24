// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Agent-discovery generators (`llms.txt`, `ai-catalog.json`, `agent-skills.json`).

use std::error::Error;

use super::metadata::{Metadata, lotus_home};

/// ARD (`Agentic Resource Discovery`) capability manifest.
/// Generated next to `llms.txt` so agents can discover the app's `WebMCP`
/// annotated search form without scraping the page.
pub fn build_ai_catalog(meta: &Metadata) -> Result<String, Box<dyn Error>> {
    let s = &meta.site;
    let base = s.base_url.trim_end_matches('/');
    let publisher = "lotus-initiative";
    let catalog = serde_json::json!({
        "specVersion": "1.0",
        "host": {
            "displayName": s.name,
            "documentationUrl": format!("{base}/llms.txt"),
            "logoUrl": format!("{base}/favicon.svg"),
        },
        "entries": [
            {
                "identifier": format!("urn:air:{publisher}:lotus-explore-rs:search"),
                "displayName": format!("{} search", s.name),
                "type": "application/agent-card+json",
                "url": format!("{base}/search"),
                "description": s.description,
                "tags": [
                    "lotus",
                    "natural-products",
                    "chemical-entities",
                    "sparql",
                    "wikidata",
                ],
                "capabilities": [
                    "SearchByTaxon",
                    "SearchByStructure",
                    "SearchByMassRange",
                    "SearchByPublicationYear",
                    "SearchByFormula",
                ],
                "representativeQueries": [
                    "Find natural products reported for Gentiana lutea",
                    "Search compounds by substructure with a molecular mass range",
                ],
                "metadata": {
                    "appLicense": s.app_license_url,
                    "dataLicense": s.data_license_url,
                    "sourceUrl": s.source_path,
                },
            },
            {
                "identifier": format!("urn:air:{publisher}:lotus-explore-rs:curation"),
                "displayName": format!("{} Wikidata curation", s.name),
                "type": "application/agent-card+json",
                "url": format!("{base}/curation"),
                "description":
                    "Import TSV rows, resolve structures and references, and generate \
                     QuickStatements for Wikidata curation.",
                "tags": ["wikidata", "curation", "quickstatements", "rdkit"],
                "capabilities": [
                    "ImportTsvRows",
                    "ResolveStructure",
                    "ResolveReference",
                    "GenerateQuickStatements",
                ],
                "representativeQueries": [
                    "Curate natural products for a taxon into Wikidata",
                    "Generate QuickStatements for a list of DOIs",
                ],
                "metadata": {
                    "appLicense": s.app_license_url,
                    "dataLicense": s.data_license_url,
                    "sourceUrl": s.source_path,
                },
            },
        ],
    });
    Ok(format!("{}\n", serde_json::to_string_pretty(&catalog)?))
}

/// Agent skills manifest for `.well-known/agent-skills.json`, keeping the
/// documented site base URL synchronized with `site-metadata.json`.
pub fn build_agent_skills(meta: &Metadata) -> Result<String, Box<dyn Error>> {
    let doc = serde_json::json!({
        "@context": "https://specification.website/contexts/agent-skills.jsonld",
        "skills": [
            {
                "name": "Search Compounds",
                "description": "Search for chemical compounds by name, SMILES, or Molfile",
                "instructions": "Use the search interface to query compounds. Support structural and text-based search, and respect the visible accessibility affordances in the page header and filters.",
                "triggers": ["compound search", "SMILES lookup", "chemical structure"],
            },
            {
                "name": "Browse Taxonomies",
                "description": "Explore plant and organism taxonomies from domain to species level",
                "instructions": "Navigate through taxonomy hierarchy or use search to find specific taxa",
                "triggers": ["taxonomy browse", "plant families", "species lookup"],
            },
            {
                "name": "Citation Lookup",
                "description": "Find references by DOI or Wikidata identifier",
                "instructions": "Enter a DOI, reference title, or Wikidata QID to retrieve publication metadata",
                "triggers": ["reference lookup", "DOI search", "reference title search"],
            },
            {
                "name": "SPARQL Query",
                "description": "Execute structured SPARQL queries against the LOTUS dataset",
                "instructions": "Write SPARQL queries to explore the compound-taxon-reference knowledge graph",
                "triggers": ["SPARQL", "structured query"],
            },
            {
                "name": "Export Results",
                "description": "Download search results as CSV, TSV, JSON, JSON-LD, or Turtle",
                "instructions": "After performing a search, use export options to download structured data",
                "triggers": ["export data", "download results"],
            },
            {
                "name": "Curation Workflows",
                "description": "Generate QuickStatements to contribute to Wikidata",
                "instructions": "Use curation tools to create structured data edits for Wikidata. Keep form labels, visible focus states, and document metadata intact when guiding the workflow.",
                "triggers": ["curation", "wikidata contribution", "quickstatements"],
            },
        ],
        "capabilities": {
            "structured_search": true,
            "taxonomy_browsing": true,
            "sparql_queries": true,
            "data_export": true,
            "wikidata_integration": true,
        },
        "data_source": "https://qlever.dev/api/wikidata",
        "documentation": meta.site.base_url,
    });
    Ok(format!("{}\n", serde_json::to_string_pretty(&doc)?))
}

/// The `WebMCP` half of the `Discovery` section of `llms.txt`, in prose.
///
/// Its own function because it is the part that changes when the tool surface
/// changes, and inlined it is most of `build_llms_txt`.
fn webmcp_discovery_notes(base: &str) -> String {
    format!(
        "Every entry above is also linked from the document head, because the one\n\
live host ignores `_headers` and a `Link:` header alone is discoverable by\n\
nobody. The `WebMCP` surface is not a separate endpoint: there is no MCP\n\
server. It is registered by the page itself, twice over. The forms carry\n\
declarative annotations -- `toolname`, `tooldescription`, and a\n\
`toolparamdescription` per control -- and `assets/js/webmcp.js` registers\n\
the same capabilities imperatively with\n\
`document.modelContext.registerTool` before the app has booted. The\n\
second registration is not redundancy: the forms are rendered by the\n\
WASM client, so the annotations do not exist until seconds after the\n\
document loaded, and an agent (or an audit) looking for tools at document\n\
start would otherwise find none.\n\n\
- `search_lotus` -- fills and submits the search form, then reports the\n\
top rows of the result table. Arguments: `taxon`, `reference`, `smiles`,\n\
`mass_min`, `mass_max`, `year_min`, `year_max`, `formula_exact`,\n\
`formula_enabled`, `stype`.\n\
- `get_lotus_search_state` -- reads the criteria currently in the form,\n\
for confirming what a previous `search_lotus` call applied.\n\
- `add_curation_row`, `import_curation_tsv` -- registered on the curation\n\
route only, because a tool whose form the agent cannot reach is context\n\
it has to spend tokens on and then fail to use.\n\n\
The tools drive the real controls, so a human watching sees the same\n\
fields fill and the same results appear. What they return comes from\n\
Wikidata and third-party services and is flagged `untrustedContentHint`.\n\n\
- Search form: {base}search\n\
- Curation forms: {base}curation\n\n"
    )
}

pub fn build_llms_txt(meta: &Metadata) -> String {
    let s = &meta.site;
    // The app is served from a subpath, so a root-relative link such as
    // /.well-known/ai-catalog.json resolves to the domain root and 404s. Every
    // URL below is absolute for that reason. `base_url` already ends in "/",
    // which is what makes `{base}docs/references.bib` correct.
    let well_known = |file: &str| format!("{}.well-known/{}", s.base_url, file);
    // `base_url` ends in "/", which is what makes `{base}search` correct here.
    let webmcp_notes = webmcp_discovery_notes(&s.base_url);
    format!(
        "# {name}\n\n\
        > {description}\n\n\
        All URLs below are absolute: the app is served from a subpath, so a\n\
        root-relative link resolves to the domain root and 404s.\n\n\
        ## Core information\n\n\
        - **Official name**: {name}\n\
        - **Short name**: {short_name}\n\
        - **Purpose**: Interactive exploration of chemical entity occurrence data\n\
        - **Data domain**: Natural products, chemical compounds, taxonomy, scientific references\n\
        - **Access model**: Free, web-based, no authentication required\n\
        - **Interface**: Four locales (en, fr, de, it); light and dark themes\n\
        - **Machine interface**: A WebMCP tool surface, see Discovery\n\n\
        ## Features\n\n\
        - Search by taxon filters and structure input (SMILES or Molfile V2000/V3000)\n\
        - Explore results as an interactive compound-taxon-reference network at {base}network\n\
        - Draw a structure in the embedded Ketcher editor, then copy Daylight SMILES or MOL V3000 back into search\n\
        - Filter by mass range, publication year, and formula presence\n\
        - Browse taxonomy and references for each result\n\
        - Export results as CSV, JSON, RDF, or SPARQL\n\
        - Import TSV rows and generate QuickStatements for Wikidata curation\n\
        - A guided FAQ at {base}faq\n\n\
        ## Docs\n\n\
        In the repository. These are design and operations notes rather than user\n\
        documentation, and are not served from the site itself.\n\n\
        - [Architecture]({repo_url}/blob/main/apps/lotus-explore-rs/docs/ARCHITECTURE.md)\n\
        - [Design system]({repo_url}/blob/main/apps/lotus-explore-rs/docs/DESIGN_SYSTEM.md)\n\
        - [Performance]({repo_url}/blob/main/apps/lotus-explore-rs/docs/PERFORMANCE.md)\n\
        - [Deployment]({repo_url}/blob/main/apps/lotus-explore-rs/docs/DEPLOYMENT.md)\n\
        - [Citation]({repo_url}/blob/main/apps/lotus-explore-rs/docs/CITATION.md)\n\
        - [Curation share links]({repo_url}/blob/main/apps/lotus-explore-rs/docs/CURATION_SHARE_LINKS.md)\n\n\
        ## Discovery\n\n\
        Machine-readable descriptions of this app, for agents.\n\n\
        - [AI capability catalog]({ai_catalog})\n\
        - [API catalog]({api_catalog})\n\
        - [Agent skills]({agent_skills})\n\
        - [Security policy]({security_txt})\n\
        - [robots.txt]({base}robots.txt)\n\
        - [sitemap.xml]({base}sitemap.xml)\n\
        - Structured data: JSON-LD in the page head\n\n\
        {webmcp_notes}## Data sources\n\n\
        - [Wikidata Query Service](https://query-next.wikidata.org/)\n\
        - [QLever Wikidata endpoint](https://qlever.dev/wikidata)\n\
        - [DOI metadata](https://doi.org/)\n\
        - [LOTUS initiative]({lotus_home_url})\n\n\
        ## Citation\n\n\
        - [Paper]({paper_landing_url})\n\
        - [DOI]({paper_doi_url})\n\
        - [BibTeX]({base}docs/references.bib)\n\n\
        ## Licensing\n\n\
        - App: [AGPL-3.0]({app_license_url})\n\
        - Wikidata data: [CC0 1.0]({data_license_url}) (application code is AGPL-3.0; external literature abstracts remain under publisher copyright)\n\n\
        ## Project\n\n\
        - [App]({base_url})\n\
        - [Repository and source]({repo_url})\n\
        - [Issues]({issues_url})\n\
        - [Discussions]({discussions_url})\n",
        name = s.name,
        short_name = s.short_name,
        description = s.description,
        base_url = s.base_url,
        base = s.base_url,
        repo_url = s.repo_url,
        issues_url = s.issues_url,
        discussions_url = s.discussions_url,
        lotus_home_url = lotus_home(meta),
        paper_landing_url = s.paper_landing_url,
        paper_doi_url = s.paper_doi_url,
        app_license_url = s.app_license_url,
        data_license_url = s.data_license_url,
        ai_catalog = well_known("ai-catalog.json"),
        api_catalog = well_known("api-catalog.json"),
        agent_skills = well_known("agent-skills.json"),
        security_txt = well_known("security.txt"),
    )
}
