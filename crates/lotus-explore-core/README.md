# lotus-explore-core

The LOTUS Explorer's domain logic, with no UI framework in it.

This crate holds the half of the application that does not know which UI
framework is drawing it: the explore state machine and its reducer, the form
validation, the URL codec, the query building, the error classification and
retry policy, the export formatting, the API wire format, and the transport
port the host implements.

The other half lives in `apps/lotus-explore-rs`: the Dioxus components, the
routing, the server functions, and the HTTP, storage and timer code. The
boundary between them is checked mechanically — `./mk boundary` fails the build
if a `dioxus` dependency reaches this crate, or if the word appears in its
sources.

## The shape of the domain

State is plain data and the update functions are pure:

```rust
use lotus_explore_core::explore::actions::ExploreAction;
use lotus_explore_core::explore::search_state::ExploreState;

let mut state = ExploreState::new(2026);
lotus_explore_core::explore::search_state::reduce_mut(
    &mut state,
    ExploreAction::ErrorDismissed,
);
```

The clock is an argument rather than a read, because a crate that cannot see
the calendar can have its tests pin one.

## License

AGPL-3.0-only.
