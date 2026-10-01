# BigQuery Storage `ReadRows` fails on responses over 4 MiB

`google-cloud-bigquery` 0.18.0's Storage Read client (`client::Read`) can't
receive a `ReadRowsResponse` larger than 4 MiB. The gRPC client keeps tonic's
default receive limit, so any table with moderately large rows fails on its
first response:

```
OUT_OF_RANGE: Error, decoded message length too large: found 9027710 bytes, the limit is: 4194304 bytes
```

The service is allowed to send much larger responses. The client's own docs for
`Read::read_rows` say:

> Each response contains one or more table rows, up to a maximum of 128 MB per
> response; read requests which attempt to read individual rows larger than
> 128 MB will fail.

The error comes from the client, so retrying or resuming the stream at the same
offset fails the same way. Nothing in `ClientBuilder` or `RequestOptions` raises
the limit.

## Reproduce

You need Application Default Credentials (`gcloud auth application-default login`)
and a project where you can create read sessions (`bigquery.readsessions.create`).
The data comes from `bigquery-public-data`, and the program reads at most three
responses, so the cost is negligible.

With [mise](https://mise.jdx.dev), which installs the pinned Rust and uv:

```sh
mise install
export BILLING_PROJECT=<billing-project>   # or set it in mise.local.toml

mise run rust           # fails: `content` holds whole source files, about 8.7 KB per row
mise run rust-control   # succeeds: the same table with the small `id` column
mise run python         # succeeds: Google's Python client reading `content`
mise run compare        # runs the Rust and Python reads of `content` back to back
```

Without mise, run `cargo run -- <billing-project> [column]` and
`uv run python/read_rows.py <billing-project> [column]`.

### Observed

```
column:  content
response 1: FAILED
  the service reports an error with code OUT_OF_RANGE described as: Error, decoded message length too large: found 9027710 bytes, the limit is: 4194304 bytes
```

```
column:  id
response 1: OK, 4096 rows, 180392 Arrow bytes (0.17 MiB)
response 2: OK, 4096 rows, 180392 Arrow bytes (0.17 MiB)
response 3: OK, 3617 rows, 159312 Arrow bytes (0.15 MiB)
```

For small rows the service sends up to 4,096 rows per response. For large rows
it sends fewer rows, and the responses observed here were 8–10 MiB each, so any
table with rows larger than about 1 KiB is likely to go over the limit.

### Expected

The read succeeds, as it does in Google's BigQuery Storage clients for other
languages, which raise the receive limit for this API. The official Python
client (`google-cloud-bigquery-storage`, in
[`python/read_rows.py`](python/read_rows.py)) reads the same column
(`mise run python`):

```
response 1: OK, 1038 rows, 9027560 Arrow bytes (8.61 MiB)
response 2: OK, 991 rows, 8601568 Arrow bytes (8.20 MiB)
response 3: OK, 872 rows, 10168312 Arrow bytes (9.70 MiB)
```

The first response holds 9,027,560 bytes of Arrow data. The Rust client rejected
a 9,027,710-byte message, which is the same response plus its protobuf wrapping.

## Cause

`google-cloud-gax-internal` 0.7.20 builds the tonic client without setting a
decoding limit (`src/grpc.rs`):

```rust
let inner_client = InnerClient::new(channel);
```

so tonic's `DEFAULT_MAX_RECV_MESSAGE_SIZE` of 4 MiB applies
(`tonic-0.14.6/src/codec/mod.rs`). A fix would set
`max_decoding_message_size` on the inner client, either for every service or as
an option on the client builder, with BigQuery Read defaulting to at least
128 MiB.

## Versions

- google-cloud-bigquery 0.18.0
- google-cloud-gax 1.15.0
- google-cloud-gax-internal 0.7.20
- tonic 0.14.6
- rustc 1.98.1
