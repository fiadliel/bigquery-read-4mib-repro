# /// script
# requires-python = ">=3.12"
# dependencies = ["google-cloud-bigquery-storage[pyarrow]"]
# ///
"""Reads the same session as the Rust program, using Google's Python client.

Usage: uv run python/read_rows.py <billing-project> [column]
"""

import sys

from google.cloud.bigquery_storage import BigQueryReadClient, types

TABLE = "projects/bigquery-public-data/datasets/github_repos/tables/sample_contents"
RESPONSES = 3


def main() -> None:
    if len(sys.argv) < 2:
        sys.exit("usage: uv run python/read_rows.py <billing-project> [column]")
    billing_project = sys.argv[1]
    column = sys.argv[2] if len(sys.argv) > 2 else "content"

    client = BigQueryReadClient()
    session = client.create_read_session(
        parent=f"projects/{billing_project}",
        read_session=types.ReadSession(
            table=TABLE,
            data_format=types.DataFormat.ARROW,
            read_options=types.ReadSession.TableReadOptions(selected_fields=[column]),
        ),
        max_stream_count=1,
    )
    print(f"table:   {TABLE}")
    print(f"column:  {column}")
    print(f"session: {session.name}")

    for index, response in enumerate(client.read_rows(session.streams[0].name), 1):
        size = len(response.arrow_record_batch.serialized_record_batch)
        print(
            f"response {index}: OK, {response.row_count} rows, "
            f"{size} Arrow bytes ({size / 1_048_576:.2f} MiB)"
        )
        if index == RESPONSES:
            break


if __name__ == "__main__":
    main()
