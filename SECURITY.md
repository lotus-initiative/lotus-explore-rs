# Security Policy

## Reporting a Vulnerability

Please report suspected security vulnerabilities through GitHub Security
Advisories rather than opening a public issue:

https://github.com/lotusnprod/lotus-explore-rs/security/advisories/new

Include steps to reproduce the issue, affected crate(s) or endpoint(s), and any
relevant configuration or input payloads.

## Supported Versions

Security fixes target the current `main` branch.

## Scope

In scope:
- SPARQL query construction and escaping (`lotus-query`)
- Input parsing and URL decoding (`lotus-model`, `lotus-explore-core`)
- Native HTTP API server handlers and SSR/static file serving (`apps/lotus-explore-rs`)
- QuickStatements and Wikidata API handling (`lotus-curation`)

Out of scope:
- Availability or rate limits of external public SPARQL endpoints (QLever, WDQS)
- Public read-only biological/chemical dataset content stored in Wikidata
