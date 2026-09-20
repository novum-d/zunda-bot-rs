use anyhow::{Context as _, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

const METADATA_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

#[derive(Clone)]
pub struct BillingClient {
    http: Client,
    query_project: String,
    dataset: String,
    table: String,
    cost_project: String,
    maximum_bytes_billed: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CostSummary {
    pub monthly: f64,
    pub yearly: f64,
    pub currency: String,
    pub exported_at: Option<String>,
}

#[derive(Deserialize)]
struct AccessToken {
    access_token: String,
}

#[derive(Deserialize)]
struct QueryResponse {
    #[serde(default, rename = "jobComplete")]
    job_complete: bool,
    #[serde(default)]
    rows: Vec<QueryRow>,
}

#[derive(Deserialize)]
struct QueryRow {
    f: Vec<QueryCell>,
}

#[derive(Deserialize)]
struct QueryCell {
    v: Option<String>,
}

impl BillingClient {
    pub fn new(
        query_project: String,
        dataset: String,
        table: String,
        cost_project: String,
        maximum_bytes_billed: u64,
    ) -> Result<Self> {
        anyhow::ensure!(
            valid_project_id(&query_project),
            "invalid billing project id"
        );
        anyhow::ensure!(valid_bigquery_id(&dataset), "invalid billing dataset id");
        anyhow::ensure!(valid_bigquery_id(&table), "invalid billing table id");
        let http = Client::builder().timeout(Duration::from_secs(20)).build()?;
        Ok(Self {
            http,
            query_project,
            dataset,
            table,
            cost_project,
            maximum_bytes_billed,
        })
    }

    pub async fn costs(&self) -> Result<CostSummary> {
        let response = self
            .http
            .post(format!(
                "https://bigquery.googleapis.com/bigquery/v2/projects/{}/queries",
                self.query_project
            ))
            .bearer_auth(self.token().await?)
            .json(&self.query_request())
            .send()
            .await?
            .error_for_status()
            .context("BigQuery billing query failed")?
            .json::<QueryResponse>()
            .await?;
        anyhow::ensure!(
            response.job_complete,
            "BigQuery billing query did not finish"
        );
        parse_cost_summary(&response)
    }

    async fn token(&self) -> Result<String> {
        Ok(self
            .http
            .get(METADATA_TOKEN_URL)
            .header("Metadata-Flavor", "Google")
            .send()
            .await?
            .error_for_status()?
            .json::<AccessToken>()
            .await?
            .access_token)
    }

    fn query_request(&self) -> Value {
        json!({
            "query": self.query(),
            "useLegacySql": false,
            "parameterMode": "NAMED",
            "queryParameters": [{
                "name": "cost_project",
                "parameterType": { "type": "STRING" },
                "parameterValue": { "value": self.cost_project }
            }],
            "maximumBytesBilled": self.maximum_bytes_billed.to_string(),
            "timeoutMs": 10_000
        })
    }

    fn query(&self) -> String {
        format!(
            r#"SELECT
  COALESCE(SUM(IF(usage_start_time >= TIMESTAMP(DATE_TRUNC(CURRENT_DATE("Asia/Tokyo"), MONTH), "Asia/Tokyo"), net_cost, 0)), 0) AS monthly_cost,
  COALESCE(SUM(net_cost), 0) AS yearly_cost,
  ANY_VALUE(currency) AS currency,
  MAX(export_time) AS exported_at
FROM (
  SELECT usage_start_time, export_time, currency,
    cost + IFNULL((SELECT SUM(credit.amount) FROM UNNEST(credits) AS credit), 0) AS net_cost
  FROM `{}.{}.{}`
  WHERE project.id = @cost_project
    AND usage_start_time >= TIMESTAMP(DATE_TRUNC(CURRENT_DATE("Asia/Tokyo"), YEAR), "Asia/Tokyo")
)"#,
            self.query_project, self.dataset, self.table
        )
    }
}

fn parse_cost_summary(response: &QueryResponse) -> Result<CostSummary> {
    let row = response
        .rows
        .first()
        .context("billing query returned no rows")?;
    anyhow::ensure!(row.f.len() == 4, "billing query returned invalid columns");
    Ok(CostSummary {
        monthly: parse_amount(&row.f[0])?,
        yearly: parse_amount(&row.f[1])?,
        currency: row.f[2].v.clone().unwrap_or_else(|| "不明".into()),
        exported_at: row.f[3].v.clone(),
    })
}

fn parse_amount(cell: &QueryCell) -> Result<f64> {
    cell.v
        .as_deref()
        .unwrap_or("0")
        .parse()
        .context("billing query returned invalid cost")
}

fn valid_project_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn valid_bigquery_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_identifiers_before_building_sql() {
        assert!(BillingClient::new(
            "billing-project".into(),
            "billing_export".into(),
            "gcp_billing_export_v1_ACCOUNT".into(),
            "game-project".into(),
            100_000_000,
        )
        .is_ok());
        assert!(BillingClient::new(
            "billing-project`".into(),
            "billing_export".into(),
            "table".into(),
            "game-project".into(),
            100_000_000,
        )
        .is_err());
    }

    #[test]
    fn parses_cost_summary_row() {
        let response: QueryResponse = serde_json::from_str(
            r#"{"jobComplete":true,"rows":[{"f":[{"v":"123.4"},{"v":"567.8"},{"v":"JPY"},{"v":"2026-08-12T03:00:00Z"}]}]}"#,
        )
        .expect("response should parse");
        assert_eq!(
            parse_cost_summary(&response).expect("summary should parse"),
            CostSummary {
                monthly: 123.4,
                yearly: 567.8,
                currency: "JPY".into(),
                exported_at: Some("2026-08-12T03:00:00Z".into()),
            }
        );
    }

    #[test]
    fn query_filters_cost_project_and_current_year() {
        let client = BillingClient::new(
            "billing-project".into(),
            "billing_export".into(),
            "gcp_billing_export_v1_ACCOUNT".into(),
            "game-project".into(),
            100_000_000,
        )
        .expect("client should build");
        let query = client.query();

        assert!(query.contains("project.id = @cost_project"));
        assert!(query.contains("DATE_TRUNC(CURRENT_DATE(\"Asia/Tokyo\"), YEAR)"));
        assert!(query.contains("DATE_TRUNC(CURRENT_DATE(\"Asia/Tokyo\"), MONTH)"));
        assert!(query.contains("SUM(credit.amount) FROM UNNEST(credits)"));
    }
}
