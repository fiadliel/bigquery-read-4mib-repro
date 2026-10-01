//! Reads the first `ReadRows` response from a public table whose rows are large.
//!
//! Usage: cargo run -- <billing-project> [column]
//!
//! `column` defaults to `content` (whole source files), which makes BigQuery send
//! responses larger than 4 MiB. Pass `id` as a control: its responses are small
//! and the read succeeds.

use google_cloud_bigquery::{client::Read, model};

const TABLE: &str = "projects/bigquery-public-data/datasets/github_repos/tables/sample_contents";
const RESPONSES: usize = 3;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let billing_project = args
        .next()
        .ok_or("usage: cargo run -- <billing-project> [column]")?;
    let column = args.next().unwrap_or_else(|| "content".to_owned());

    let client = Read::builder().build().await?;

    let session = client
        .create_read_session()
        .set_parent(format!("projects/{billing_project}"))
        .set_read_session(
            model::ReadSession::new()
                .set_table(TABLE)
                .set_data_format(model::DataFormat::Arrow)
                .set_read_options(
                    model::read_session::TableReadOptions::new().set_selected_fields([&column]),
                ),
        )
        .set_max_stream_count(1)
        .send()
        .await?;
    let stream = session
        .streams
        .first()
        .ok_or("the read session returned no streams")?
        .name
        .clone();
    println!("table:   {TABLE}");
    println!("column:  {column}");
    println!("session: {}", session.name);

    let mut rows = client.read_rows().set_read_stream(stream).send().await?;
    for index in 1..=RESPONSES {
        match rows.next().await {
            Some(Ok(response)) => {
                let bytes = match &response.rows {
                    Some(model::read_rows_response::Rows::ArrowRecordBatch(batch)) => {
                        batch.serialized_record_batch.len()
                    }
                    _ => 0,
                };
                println!(
                    "response {index}: OK, {} rows, {bytes} Arrow bytes ({:.2} MiB)",
                    response.row_count,
                    bytes as f64 / 1_048_576.0
                );
            }
            Some(Err(error)) => {
                println!("response {index}: FAILED");
                println!("  {error}");
                return Err(error.into());
            }
            None => {
                println!("stream exhausted");
                break;
            }
        }
    }
    Ok(())
}
