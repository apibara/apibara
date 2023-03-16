# Apibara StarkNet DNA Server

This crate provides `apibara-starknet`, used to start a source node that
streams data from StarkNet.


## Running

The node can export data to any service that can ingest OpenTelemetry data.
When developing locally, you can run a service with:

```
docker run --rm -p 4317:4317 otel/opentelemetry-collector-dev:latest
```

The docker container will periodically output the metric and trace data.


## Testing

You can run unit tests with:

```
cargo test -p apibara-starknet --lib
```

