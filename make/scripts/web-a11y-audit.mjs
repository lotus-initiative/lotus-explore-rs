#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//
// Optional headless Chromium + axe-core responsive and accessibility check.
// Uses zero repository Node dependencies: supply `playwright-core` and `axe-core`
// via `LOTUS_A11Y_NODE_PATH` or `NODE_PATH` (or a local `node_modules`).

import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

function resolveExternalModule(specifier) {
  const candidates = [];
  if (process.env.LOTUS_A11Y_NODE_PATH) {
    for (const dir of process.env.LOTUS_A11Y_NODE_PATH.split(path.delimiter).filter(Boolean)) {
      candidates.push(dir);
    }
  }
  if (process.env.NODE_PATH) {
    for (const dir of process.env.NODE_PATH.split(path.delimiter).filter(Boolean)) {
      candidates.push(dir);
    }
  }
  candidates.push(process.cwd(), path.dirname(new URL(import.meta.url).pathname));

  for (const base of candidates) {
    const req = createRequire(path.join(path.resolve(base), "__resolve__.cjs"));
    try {
      return req.resolve(specifier);
    } catch {
      // Try next search root
    }
  }
  return null;
}

const pwEntry = resolveExternalModule("playwright-core");
const axePath = resolveExternalModule("axe-core/axe.min.js");
if (!pwEntry || !axePath) {
  console.error(
    [
      "web-a11y-audit: missing `playwright-core` or `axe-core`.",
      "Set LOTUS_A11Y_NODE_PATH (or NODE_PATH) to a `node_modules` directory",
      "containing both packages, e.g.:",
      "  LOTUS_A11Y_NODE_PATH=/path/to/node_modules ./mk web-a11y",
    ].join("\n")
  );
  process.exit(2);
}

const pwMod = await import(pathToFileURL(pwEntry).href);
const chromium = pwMod.chromium ?? pwMod.default?.chromium;

function findChrome() {
  const candidates = [
    process.env.CHROME_PATH,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ].filter(Boolean);
  for (const p of candidates) {
    if (fs.existsSync(p)) return p;
  }
  return undefined;
}

function parseArgs(argv) {
  const args = {};
  for (let i = 2; i < argv.length; i++) {
    const key = argv[i];
    if (key.startsWith("--")) {
      const name = key.slice(2);
      const next = argv[i + 1];
      if (next && !next.startsWith("--")) {
        args[name] = next;
        i++;
      } else {
        args[name] = "true";
      }
    }
  }
  return args;
}

const MIME_TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "application/javascript; charset=utf-8",
  ".mjs": "application/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".wasm": "application/wasm",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".json": "application/json; charset=utf-8",
  ".ico": "image/x-icon",
};

const MOCK_ROWS = [
  {
    compound_qid: "Q27102658",
    name: "Gentiopicroside",
    inchikey: "DUZRGZMVJBSGRR-UHFFFAOYSA-N",
    smiles: "C=CC1C(C(=COC1OC2C(C(C(C(O2)CO)O)O)O)C(=O)OC3=CC=CC=C3)O",
    mass: 356.1107,
    formula: "C16H20O9",
    taxon_qid: "Q158572",
    taxon_name: "Gentiana lutea",
    reference_qid: "Q57921011",
    ref_title:
      "Phytochemical profiling and secoiridoid glycosides of Gentiana lutea roots across Alpine populations",
    ref_doi: "10.1016/j.phytochem.2020.112345",
    pub_year: 2020,
    statement: "Q27102658$11111111-2222-3333-4444-555555555551",
  },
  {
    compound_qid: "Q27115644",
    name: "Amarogentin",
    inchikey: "LVBPLWLMKQQDJS-UHFFFAOYSA-N",
    smiles:
      "C=CC1C(C(=COC1OC2C(C(C(C(O2)CO)O)O)OC(=O)C3=CC(=CC(=C3)O)C4=CC=CC=C4O)C(=O)OCC1)O",
    mass: 586.1686,
    formula: "C29H30O13",
    taxon_qid: "Q159234",
    taxon_name: "Gentiana punctata",
    reference_qid: "Q57921013",
    ref_title:
      "Bitter secoiridoid glycosides in European Gentianaceae: comparative HPLC-MS/MS analysis",
    ref_doi: "10.1021/acs.jnatprod.9b00421",
    pub_year: 2019,
    statement: "Q27115644$11111111-2222-3333-4444-555555555555",
  },
];

const MOCK_QUERY =
  'SELECT DISTINCT ?compound ?compoundLabel WHERE { ?taxon wdt:P225 "Gentiana lutea" . } LIMIT 1000';
const MOL_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="180" height="120"><rect width="180" height="120" fill="#fff"/></svg>';
const BADGE_LIGHT_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="118" height="28"><rect width="118" height="28" fill="#e7ecf1"/><text x="59" y="18" text-anchor="middle" font-size="11" fill="#4a5a6d">Simple Analytics</text></svg>';
const BADGE_DARK_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="118" height="28"><rect width="118" height="28" fill="#1c2637"/><text x="59" y="18" text-anchor="middle" font-size="11" fill="#b8c5d6">Simple Analytics</text></svg>';
const TRANSPARENT_GIF = Buffer.from(
  "R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7",
  "base64"
);

function startStaticAndMockServer(distDir) {
  const indexHtmlPath = path.join(distDir, "index.html");
  const server = http.createServer((req, res) => {
    const parsed = new URL(req.url || "/", "http://127.0.0.1");
    const pathname = decodeURIComponent(parsed.pathname);

    if (req.method === "POST" && pathname === "/v1/search") {
      let body = "";
      req.on("data", (c) => {
        body += c;
      });
      req.on("end", () => {
        let payload = {};
        try {
          payload = JSON.parse(body || "{}");
        } catch {
          payload = {};
        }
        if (String(payload.taxon || "").includes("ERROR_TEST")) {
          res.writeHead(500, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ error: "Simulated audit timeout" }));
          return;
        }
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(
          JSON.stringify({
            resolved_taxon_qid: "Q158572",
            warning: "Resolved synonym 'Gentiana lutea' to Q158572",
            query: MOCK_QUERY,
            total_matches: MOCK_ROWS.length,
            rows: MOCK_ROWS,
          })
        );
      });
      return;
    }

    const rel = pathname.replace(/^\/+/, "");
    let filePath = path.resolve(distDir, rel);
    if (!filePath.startsWith(path.resolve(distDir))) {
      res.writeHead(403);
      res.end("Forbidden");
      return;
    }
    if (fs.existsSync(filePath) && fs.statSync(filePath).isDirectory()) {
      const idx = path.join(filePath, "index.html");
      if (fs.existsSync(idx)) filePath = idx;
    }
    if (!fs.existsSync(filePath) || !fs.statSync(filePath).isFile()) {
      const ext = path.extname(pathname).toLowerCase();
      if (!pathname.startsWith("/assets/") && (!ext || ext === ".html")) {
        filePath = indexHtmlPath;
      } else {
        res.writeHead(404);
        res.end("Not found");
        return;
      }
    }
    const ext = path.extname(filePath).toLowerCase();
    res.writeHead(200, {
      "Content-Type": MIME_TYPES[ext] || "application/octet-stream",
    });
    fs.createReadStream(filePath).pipe(res);
  });

  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const addr = server.address();
      resolve({ server, origin: `http://127.0.0.1:${addr.port}` });
    });
  });
}

async function setupInterception(context) {
  await context.route(/^https?:\/\/(?!127\.0\.0\.1)/, async (route) => {
    const url = route.request().url();
    if (url.includes("simolecule.com")) {
      await route.fulfill({ status: 200, contentType: "image/svg+xml", body: MOL_SVG });
    } else if (url.includes("simpleanalyticsbadges.com")) {
      const dark = url.includes("mode=dark");
      await route.fulfill({
        status: 200,
        contentType: "image/svg+xml",
        body: dark ? BADGE_DARK_SVG : BADGE_LIGHT_SVG,
      });
    } else if (url.includes("queue.simpleanalyticscdn.com")) {
      await route.fulfill({ status: 200, contentType: "image/gif", body: TRANSPARENT_GIF });
    } else if (
      url.includes("qlever.dev") ||
      url.includes("query.wikidata.org") ||
      url.includes("query-next.wikidata.org") ||
      url.includes("query-scholarly-next.wikidata.org") ||
      url.includes("idsm.elixir-czech.cz")
    ) {
      await route.fulfill({ status: 429, contentType: "text/plain", body: "Simulated 429" });
    } else {
      await route.fulfill({ status: 404, contentType: "text/plain", body: "Blocked" });
    }
  });
}

async function waitMounted(page) {
  await page.waitForFunction(() => {
    const boot = document.querySelector("[data-boot-shell]");
    return (
      !boot &&
      document.querySelector("header") !== null &&
      document.querySelector("main") !== null &&
      document.querySelector("footer") !== null
    );
  }, { timeout: 25000 });
  await page.waitForFunction(() => {
    const css = document.getElementById("app-css");
    return css && css.media === "all" && css.sheet !== null;
  }, { timeout: 3000 });
}

function withParams(origin, routePath, params) {
  const url = new URL(routePath, origin);
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== null) url.searchParams.set(k, v);
  }
  return url.toString();
}

async function visitScenario(page, origin, scenario, theme) {
  const base = { dark_mode: theme === "dark" ? "true" : "false" };
  if (scenario === "landing") {
    await page.goto(withParams(origin, "/", base), { waitUntil: "domcontentloaded" });
    await waitMounted(page);
    await page.waitForSelector("#lotus-search-form");
  } else if (scenario === "search-expanded") {
    await page.goto(withParams(origin, "/search", base), { waitUntil: "domcontentloaded" });
    await waitMounted(page);
    await page.evaluate(() => {
      document.querySelectorAll("main details:not([open]) > summary").forEach((s) => s.click());
    });
  } else if (scenario === "search-results") {
    await page.goto(
      withParams(origin, "/search", {
        ...base,
        taxon: "Gentiana lutea",
        execute: "true",
        api_base: origin,
      }),
      { waitUntil: "domcontentloaded" }
    );
    await waitMounted(page);
    await page.waitForSelector("#results-section tr.data-row", { timeout: 20000 });
    await page.evaluate(() => {
      document.querySelectorAll("section.query-panel details:not([open]) > summary").forEach((s) => s.click());
    });
  } else if (scenario === "search-error") {
    await page.goto(
      withParams(origin, "/search", {
        ...base,
        taxon: "ERROR_TEST",
        execute: "true",
        api_base: origin,
      }),
      { waitUntil: "domcontentloaded" }
    );
    await waitMounted(page);
    await page.waitForSelector('[role="alert"]', { timeout: 20000 });
  } else if (scenario === "network") {
    await page.goto(withParams(origin, "/network", base), { waitUntil: "domcontentloaded" });
    await waitMounted(page);
    await page.waitForSelector("#network-quick-search-input");
  } else if (scenario === "network-loaded") {
    await page.goto(
      withParams(origin, "/search", {
        ...base,
        taxon: "Gentiana lutea",
        execute: "true",
        api_base: origin,
      }),
      { waitUntil: "domcontentloaded" }
    );
    await waitMounted(page);
    await page.waitForSelector("#results-section tr.data-row", { timeout: 20000 });
    await page.locator("#results-section button", { hasText: /View in Network/i }).click();
    await page.waitForSelector("#lotus-network-canvas", { timeout: 20000 });
  } else if (scenario === "curation") {
    await page.goto(withParams(origin, "/curation", base), { waitUntil: "domcontentloaded" });
    await waitMounted(page);
    await page.locator("#lotus-curation-add-row-form button[type='button']").first().click();
    await page.waitForSelector("#curation-share-url-field");
  } else if (scenario === "draw") {
    await page.goto(withParams(origin, "/draw", base), { waitUntil: "domcontentloaded" });
    await waitMounted(page);
    await page.waitForSelector("#draw-page-heading");
  } else if (scenario === "faq") {
    await page.goto(withParams(origin, "/faq", base), { waitUntil: "domcontentloaded" });
    await waitMounted(page);
    await page.evaluate(() => {
      document.querySelectorAll("main details:not([open]) > summary").forEach((s) => s.click());
    });
  } else if (scenario === "not-found") {
    await page.goto(withParams(origin, "/does-not-exist", base), { waitUntil: "domcontentloaded" });
    await waitMounted(page);
    await page.waitForSelector("#not-found-heading");
  }
  await page.waitForTimeout(200);
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
}

async function checkDom(page) {
  return page.evaluate(() => {
    const innerWidth = window.innerWidth;
    const overflowPx = Math.max(0, document.documentElement.scrollWidth - innerWidth);

    function desc(el) {
      let s = el.tagName.toLowerCase();
      if (el.id) s += `#${el.id}`;
      return s;
    }
    function isVis(el) {
      let cur = el;
      while (cur && cur.nodeType === 1) {
        const st = getComputedStyle(cur);
        if (st.display === "none" || st.visibility === "hidden") return false;
        if (cur.tagName === "DETAILS" && !cur.hasAttribute("open") && !el.closest("summary")) return false;
        cur = cur.parentElement;
      }
      const r = el.getBoundingClientRect();
      return !(r.width === 0 && r.height === 0) && !String(el.className || "").split(/\s+/).includes("sr-only");
    }
    function hasScrollX(el) {
      let cur = el.parentElement;
      while (cur && cur !== document.body && cur !== document.documentElement) {
        const st = getComputedStyle(cur);
        if ((st.overflowX === "auto" || st.overflowX === "scroll") && cur.scrollWidth > cur.clientWidth + 1) return true;
        cur = cur.parentElement;
      }
      return false;
    }

    const vpOverflow = [];
    const badScrollBoxes = [];
    for (const el of document.body.querySelectorAll("*")) {
      const cls = String(el.className || "");
      if (cls.includes("analytics-pixel") || cls.includes("skip-link") || cls.includes("lotus-graph-label") || el instanceof SVGElement) continue;
      if (!isVis(el)) continue;
      const r = el.getBoundingClientRect();
      const st = getComputedStyle(el);
      if (!hasScrollX(el) && (r.right > innerWidth + 1 || r.left < -1)) {
        vpOverflow.push(`${desc(el)} (${Math.round(r.left)}..${Math.round(r.right)} > ${innerWidth})`);
      }
      if ((st.overflowX === "auto" || st.overflowX === "scroll") && el.scrollWidth > el.clientWidth + 2) {
        const ti = el.getAttribute("tabindex");
        const named = Boolean(el.getAttribute("aria-label") || el.getAttribute("aria-labelledby"));
        if (ti === null || Number(ti) < 0 || !named || st.scrollbarWidth === "none") {
          badScrollBoxes.push(desc(el));
        }
      }
    }

    const under24 = [];
    const under44 = [];
    for (const el of document.querySelectorAll('a, button, input:not([type="hidden"]), select, textarea, summary, [role="button"], [role="switch"]')) {
      if (String(el.className || "").includes("skip-link") || !isVis(el)) continue;
      const r = el.getBoundingClientRect();
      const tag = el.tagName.toLowerCase();
      const t = el.getAttribute("type") || "";
      if ((r.width < 23.9 || r.height < 23.9) && !el.closest("p, dd, li")) {
        under24.push(`${desc(el)} (${r.width.toFixed(1)}x${r.height.toFixed(1)})`);
      }
      const isNativeChoice = tag === "input" && (t === "checkbox" || t === "radio") && Boolean(el.closest("label"));
      if (!isNativeChoice && (r.width < 43.9 || r.height < 43.9) && !el.closest("p, dd, li")) {
        under44.push(`${desc(el)} (${r.width.toFixed(1)}x${r.height.toFixed(1)})`);
      }
    }

    const inputsUnder16 = [];
    for (const el of document.querySelectorAll("input:not([type='hidden']), select, textarea")) {
      if (!isVis(el)) continue;
      const t = el.getAttribute("type") || "";
      if (t === "checkbox" || t === "radio" || t === "range") continue;
      const fsPx = parseFloat(getComputedStyle(el).fontSize);
      if (fsPx < 16) inputsUnder16.push(`${desc(el)} (${fsPx}px)`);
    }

    const proseUnder16 = [];
    if (parseFloat(getComputedStyle(document.body).fontSize) < 16) proseUnder16.push("body");
    for (const el of document.querySelectorAll("p, dd")) {
      if (!isVis(el) || !(el.innerText || "").trim()) continue;
      const fsPx = parseFloat(getComputedStyle(el).fontSize);
      if (fsPx < 16) proseUnder16.push(`${desc(el)} (${fsPx}px)`);
    }

    const css = document.getElementById("app-css");
    const cssLoaded = Boolean(css && css.media === "all" && css.sheet);
    const headerInsideMain = Boolean(document.querySelector("main header"));
    const h1Count = document.querySelectorAll("h1").length;
    const title = document.title;

    return {
      overflowPx,
      vpOverflow,
      badScrollBoxes,
      under24,
      under44,
      inputsUnder16,
      proseUnder16,
      cssLoaded,
      headerInsideMain,
      h1Count,
      title,
    };
  });
}

async function main() {
  const args = parseArgs(process.argv);
  const quick = args.quick === "true";
  const distDir = path.resolve(
    args.dist || "target/dx/lotus-explore-rs/debug/web/public"
  );
  if (!fs.existsSync(path.join(distDir, "index.html"))) {
    console.error(`web-a11y-audit: missing built bundle at ${distDir}. Run 'cd apps/lotus-explore-rs && dx build --platform web' first.`);
    process.exit(2);
  }

  const scenarios = quick
    ? ["landing", "search-results", "network-loaded", "curation", "faq", "not-found"]
    : [
        "landing",
        "search-expanded",
        "search-results",
        "search-error",
        "network",
        "network-loaded",
        "curation",
        "draw",
        "faq",
        "not-found",
      ];
  const viewports = quick
    ? [{ width: 320, height: 568 }, { width: 1280, height: 800 }]
    : [
        { width: 320, height: 568 },
        { width: 375, height: 667 },
        { width: 768, height: 1024 },
        { width: 1280, height: 800 },
      ];
  const themes = ["light", "dark"];

  const { server, origin } = await startStaticAndMockServer(distDir);
  const browser = await chromium.launch({
    executablePath: findChrome(),
    headless: true,
    args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-webgl", "--ignore-gpu-blocklist", "--no-sandbox"],
  });

  const failures = [];
  let checks = 0;

  try {
    for (const theme of themes) {
      for (const vp of viewports) {
        const ctx = await browser.newContext({ viewport: vp, colorScheme: theme });
        await setupInterception(ctx);
        const page = await ctx.newPage();
        for (const scenario of scenarios) {
          checks++;
          await visitScenario(page, origin, scenario, theme);
          const dom = await checkDom(page);
          await page.addScriptTag({ path: axePath });
          const axe = await page.evaluate(async () => {
            const res = await window.axe.run(document, {
              runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"] },
            });
            return res.violations.map((v) => `${v.id}(${v.impact},${v.nodes.length})`);
          });
          const label = `${scenario}@${vp.width}/${theme}`;
          if (dom.overflowPx > 0) failures.push(`${label}: overflowPx=${dom.overflowPx}`);
          if (dom.vpOverflow.length) failures.push(`${label}: vpOverflow=${dom.vpOverflow.slice(0, 3).join("; ")}`);
          if (dom.badScrollBoxes.length) failures.push(`${label}: badScrollBoxes=${dom.badScrollBoxes.join(",")}`);
          if (dom.under24.length) failures.push(`${label}: under24=${dom.under24.join(",")}`);
          if (dom.under44.length) failures.push(`${label}: under44=${dom.under44.join(",")}`);
          if (dom.inputsUnder16.length) failures.push(`${label}: inputsUnder16=${dom.inputsUnder16.join(",")}`);
          if (dom.proseUnder16.length) failures.push(`${label}: proseUnder16=${dom.proseUnder16.join(",")}`);
          if (!dom.cssLoaded) failures.push(`${label}: css not loaded`);
          if (dom.headerInsideMain) failures.push(`${label}: header inside main`);
          if (dom.h1Count !== 1) failures.push(`${label}: h1Count=${dom.h1Count}`);
          if (!dom.title) failures.push(`${label}: empty title`);
          if (axe.length) failures.push(`${label}: axe violations=${axe.join(", ")}`);
        }
        await ctx.close();
      }
    }
  } finally {
    await browser.close();
    server.close();
  }

  console.log(`web-a11y-audit: completed ${checks} checks with ${failures.length} failure(s).`);
  if (failures.length) {
    for (const f of failures) console.error(`  FAIL ${f}`);
    process.exit(1);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
