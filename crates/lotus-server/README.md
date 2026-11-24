# lotus-server

Native Axum `OpenAPI` HTTP API server and static SPA host for the LOTUS Explorer.

Exposes:

- `GET /health` and `GET /metrics` for runtime observability and Prometheus scraping
- `GET /openapi.json` `OpenAPI` 3 specification
- `POST /v1/search` for bounded, single-flight coalesced search execution
- `POST /v1/export-url` and `GET /v1/export-file/{cache_key}/{format}` for streaming downloads
- Optional precompressed static asset serving when `PUBLIC_DIR` is configured

```sh
cargo run --locked -p lotus-server --bin lotus-server
```

## License

AGPL-3.0-only.
