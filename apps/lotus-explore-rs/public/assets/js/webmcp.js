// WebMCP tool registration, imperative API.
//
// The declarative annotations on the forms (`toolname`, `tooldescription`,
// `toolparamdescription` -- see search_panel.rs and
// data_curation_page/sections/mod.rs) are the human-readable spec and they stay.
// They are not enough on their own, for a timing reason that is worth stating
// here rather than rediscovering: the forms are rendered by the WASM client, so
// the annotations do not exist until the app has booted, seconds after the
// document loaded. Anything that looks for tools at document start -- the
// Lighthouse `webmcp-*` audits, among others -- sees nothing and concludes the
// page has no tool surface. Registering here, from a deferred script in the
// head, means the tools are known before the app has rendered.
//
// Everything below drives the real controls rather than reimplementing a search,
// so a human watching the agent sees the same fields fill and the same results
// appear. Two details make that work against a controlled framework:
//
//   * values are written through the native prototype setter, because assigning
//     `el.value` leaves Dioxus's own value tracking believing the field is
//     unchanged, and
//   * an `input`/`change` event is dispatched afterwards, because Dioxus keeps
//     the criteria in component state and the re-render is driven by the event,
//     not by the DOM mutation.
//
// Nothing here throws into the page: a browser without WebMCP, a registration
// refused by permissions policy, and a tool whose form never appears are all
// ordinary states here, not errors.
(function () {
  "use strict";

  var context = document.modelContext;
  if (!context || typeof context.registerTool !== "function") return;

  // The app boots asynchronously, so a tool can be asked to act on a form that
  // does not exist yet. Polling rather than a MutationObserver: this runs a
  // handful of times per page and the deadline is the only state worth keeping.
  var READY_TIMEOUT_MS = 20000;
  var POLL_INTERVAL_MS = 100;

  function whenReady(selector) {
    return new Promise(function (resolve, reject) {
      var deadline = Date.now() + READY_TIMEOUT_MS;
      (function poll() {
        var element = document.querySelector(selector);
        if (element) {
          resolve(element);
          return;
        }
        if (Date.now() > deadline) {
          reject(new Error(selector + " did not appear; is this the right route?"));
          return;
        }
        setTimeout(poll, POLL_INTERVAL_MS);
      })();
    });
  }

  function fire(element, types) {
    types.forEach(function (type) {
      element.dispatchEvent(new Event(type, { bubbles: true }));
    });
  }

  function writeValue(element, value) {
    var prototype = element instanceof HTMLTextAreaElement
      ? HTMLTextAreaElement.prototype
      : HTMLInputElement.prototype;
    var setter = Object.getOwnPropertyDescriptor(prototype, "value").set;
    setter.call(element, value == null ? "" : String(value));
    fire(element, ["input", "change"]);
  }

  function setChecked(element, checked) {
    element.checked = checked;
    fire(element, ["change"]);
  }

  // The advanced filters live in closed <details>, so a field the tool was asked
  // to set does not exist until the disclosure is open. Opening them first is
  // also what the user would have to do, and it keeps the fill visible.
  function openDisclosures(root) {
    var details = root.querySelectorAll("details");
    for (var i = 0; i < details.length; i += 1) details[i].open = true;
  }

  function setField(selector, value) {
    var element = document.querySelector(selector);
    if (!element || value === undefined || value === null) return false;
    writeValue(element, value);
    return true;
  }

  function checkField(selector, checked) {
    var element = document.querySelector(selector);
    if (!element || checked === undefined || checked === null) return false;
    setChecked(element, Boolean(checked));
    return true;
  }

  function chooseRadio(value) {
    var radios = document.querySelectorAll('input[name="stype"]');
    for (var i = 0; i < radios.length; i += 1) {
      if (radios[i].value === value) {
        setChecked(radios[i], true);
        return true;
      }
    }
    return false;
  }

  // The results table is virtualised: only the rows in view are in the DOM. What
  // comes back is therefore a sample of the top of the result set, not a count
  // of everything, and says so rather than letting the agent read a small
  // number as the whole answer.
  function summariseResults() {
    var rows = document.querySelectorAll("table tbody tr");
    var lines = [];
    for (var i = 0; i < rows.length && i < 10; i += 1) {
      var text = (rows[i].textContent || "").replace(/\s+/g, " ").trim();
      if (text) lines.push("- " + text);
    }
    if (!lines.length) return "No result rows are on screen.";
    return (
      "First rows of the result table (the table is virtualised, so this is the " +
      "top of the result set rather than all of it):\n" +
      lines.join("\n")
    );
  }

  function submitAndSummarise(form, message) {
    // requestSubmit, not submit: it fires the submit event, so the framework's
    // own handler runs. A bare `form.submit()` would be a silent no-op here.
    if (typeof form.requestSubmit === "function") form.requestSubmit();
    else form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    return message;
  }

  function register(tool) {
    try {
      var result = context.registerTool(tool);
      if (result && typeof result.catch === "function") {
        result.catch(function () {
          /* Refused by permissions policy: the declarative annotations remain. */
        });
      }
    } catch (error) {
      /* Same: no imperative registration, declarative still there. */
    }
  }

  var SEARCH_SCHEMA = {
    type: "object",
    properties: {
      taxon: {
        type: "string",
        description: "Taxon scientific name, common name, Wikidata QID, or * for all taxa."
      },
      reference: {
        type: "string",
        description: "Reference as a Wikidata QID or a DOI."
      },
      smiles: {
        type: "string",
        description:
          "Structure, compound name or InChIKey. A name or InChIKey is resolved to its Wikidata compound; anything else is sent to the structure service as written."
      },
      mass_min: { type: "number", description: "Minimum molecular mass in Da." },
      mass_max: { type: "number", description: "Maximum molecular mass in Da." },
      year_min: { type: "integer", description: "Minimum publication year." },
      year_max: { type: "integer", description: "Maximum publication year." },
      formula_exact: {
        type: "string",
        description: "Exact molecular formula filter, e.g. C7H5O5N."
      },
      formula_enabled: {
        type: "boolean",
        description: "Whether the formula filter is applied."
      },
      stype: {
        type: "string",
        enum: ["exact", "substructure", "similarity"],
        description:
          "How the resolved compound is searched: that compound only, or every compound containing / similar to it."
      }
    },
    additionalProperties: true
  };

  register({
    name: "search_lotus",
    description:
      "Search LOTUS natural-product occurrences by taxon, structure or compound name, reference, mass range, publication year and formula, and show the resulting triples. Use this for any question of the form \"which compounds were reported for X\".",
    inputSchema: SEARCH_SCHEMA,
    annotations: {
      // It runs a read query, but it does change what the page shows, so this is
      // not readOnly; nothing outside the browser is modified, so it is not
      // consequential. The results come from Wikidata and third-party services,
      // which is what untrustedContentHint is for.
      readOnlyHint: false,
      consequentialHint: false,
      untrustedContentHint: true
    },
    execute: function (input) {
      return whenReady("#lotus-search-form").then(function (form) {
        openDisclosures(form);
        setField("#taxon-input", input.taxon);
        setField("#reference-input", input.reference);
        setField("#smiles-input", input.smiles);
        setField("#mass-min", input.mass_min);
        setField("#mass-max", input.mass_max);
        setField("#year-min", input.year_min);
        setField("#year-max", input.year_max);
        setField("#formula-exact", input.formula_exact);
        checkField("#formula-enabled", input.formula_enabled);
        if (input.stype) chooseRadio(input.stype);
        submitAndSummarise(form, "Search submitted.");
        return new Promise(function (resolve) {
          // The query is a network round trip to QLever, so a fixed delay would
          // either be too short to see rows or too long to be worth waiting for.
          // Poll for the first rows and report whatever is there.
          var deadline = Date.now() + 30000;
          (function poll() {
            if (document.querySelector("table tbody tr")) {
              resolve(summariseResults());
              return;
            }
            if (Date.now() > deadline) {
              resolve("The search was submitted but no result rows appeared within 30s.");
              return;
            }
            setTimeout(poll, 250);
          })();
        });
      });
    }
  });

  register({
    name: "get_lotus_search_state",
    description:
      "Read the search form's current criteria without running a search. Use it to check what a previous search_lotus call actually applied.",
    inputSchema: { type: "object", properties: {}, additionalProperties: false },
    annotations: { readOnlyHint: true, consequentialHint: false, untrustedContentHint: false },
    execute: function () {
      return whenReady("#lotus-search-form").then(function (form) {
        var read = function (selector) {
          var element = form.querySelector(selector);
          return element ? element.value : null;
        };
        var state = {
          taxon: read("#taxon-input"),
          reference: read("#reference-input"),
          smiles: read("#smiles-input"),
          mass_min: read("#mass-min"),
          mass_max: read("#mass-max"),
          year_min: read("#year-min"),
          year_max: read("#year-max"),
          formula_enabled: (form.querySelector("#formula-enabled") || {}).checked,
          formula_exact: read("#formula-exact"),
          stype: null
        };
        var radios = form.querySelectorAll('input[name="stype"]');
        for (var i = 0; i < radios.length; i += 1) {
          if (radios[i].checked) state.stype = radios[i].value;
        }
        return JSON.stringify(state, null, 2);
      });
    }
  });

  // The curation tools change the curation queue, so they are registered only on
  // the curation route: a tool for a form the agent cannot reach is context an
  // agent has to spend tokens on and then fail to use.
  if (/\/curation\/?$/.test(window.location.pathname)) {
    register({
      name: "add_curation_row",
      description:
        "Add one curated compound record (name, SMILES, taxon, optional DOI) to the Wikidata curation queue. This changes the queue but publishes nothing.",
      inputSchema: {
        type: "object",
        properties: {
          name: { type: "string", description: "Compound name." },
          smiles: { type: "string", description: "SMILES representation of the structure." },
          taxon: { type: "string", description: "Taxon name or identifier." },
          doi: { type: "string", description: "Optional DOI of the source publication." }
        },
        required: ["name", "smiles"],
        additionalProperties: true
      },
      annotations: {
        readOnlyHint: false,
        consequentialHint: false,
        untrustedContentHint: true
      },
      execute: function (input) {
        return whenReady("#lotus-curation-add-row-form").then(function (form) {
          setField("#curation-name-input", input.name);
          setField("#curation-smiles-input", input.smiles);
          setField("#curation-taxon-input", input.taxon);
          setField("#curation-doi-input", input.doi);
          return submitAndSummarise(form, "Row added to the curation queue.");
        });
      }
    });

    register({
      name: "import_curation_tsv",
      description:
        "Paste TSV rows (name, SMILES, taxon, DOI columns) into the curation import panel and parse them into the queue. This changes the queue but publishes nothing.",
      inputSchema: {
        type: "object",
        properties: {
          tsv: {
            type: "string",
            description: "TSV rows with name, SMILES, taxon and DOI columns."
          }
        },
        required: ["tsv"],
        additionalProperties: true
      },
      annotations: {
        readOnlyHint: false,
        consequentialHint: false,
        untrustedContentHint: true
      },
      execute: function (input) {
        return whenReady("#lotus-curation-tsv-form").then(function (form) {
          setField("#curation-tsv-input", input.tsv);
          return submitAndSummarise(form, "TSV parsed into the curation queue.");
        });
      }
    });
  }
})();