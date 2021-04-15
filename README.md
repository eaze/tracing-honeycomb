[![tracing-honeycomb on crates.io](https://img.shields.io/crates/v/tracing-honeycomb)](https://crates.io/crates/tracing-honeycomb)
[![Documentation (latest release)](https://docs.rs/tracing-honeycomb/badge.svg)](https://docs.rs/tracing-honeycomb/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](../LICENSE)

# tracing-honeycomb

This repo contains the source code for:
- [`tracing-distributed`](tracing-distributed/README.md), which contains generic machinery for publishing distributed trace telemetry to arbitrary backends
- [`tracing-honeycomb`](tracing-honeycomb/README.md), which contains a concrete implementation that uses [honeycomb.io](https://honeycomb.io) as a backend

## Usage

See [`tracing-honeycomb`](tracing-honeycomb/README.md) for examples.

## Credits

The `tracing-honeycomb` and `tracing-distributed` crates originate from [inanna-malick/tracing-honeycomb](https://github.com/inanna-malick/tracing-honeycomb), 
where the original author, [Inanna Malick](https://github.com/inanna-malick), did great work getting these crates in usable condition.

This repository is now the authoritative source of these two crates.

## License

Licensed under the [MIT License](LICENSE.md)

