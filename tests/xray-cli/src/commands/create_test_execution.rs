use anyhow::Result;
use clap::Args;
use reqwest::Client;
use reqwest::multipart::{Form, Part};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::path::Path;
use tokio::fs::read;

#[derive(Args)]
pub struct CreateTestExecutionArgs {
	#[arg(short, long)]
	pub input: String,
	#[arg(long)]
	pub name: String,
	#[arg(long = "test-plan-key")]
	pub test_plan_key: Option<String>,
	#[arg(long = "env")]
	pub environments: Vec<String>,
	#[arg(long = "label")]
	pub labels: Vec<String>,
}

#[derive(Serialize)]
struct XrayAuthBody {
	client_id: String,
	client_secret: String,
}

pub async fn xray_cloud_auth_token(
	client: &Client,
	client_id: String,
	client_secret: String,
) -> Result<String> {
	let resp = client
		.post("https://eu.xray.cloud.getxray.app/api/v2/authenticate")
		.json(&XrayAuthBody { client_id, client_secret })
		.send()
		.await?;

	let status = resp.status();
	let text = resp.text().await?;

	if !status.is_success() {
		anyhow::bail!("Xray auth failed: {} - {}", status, text);
	}

	let token = text.trim().trim_matches('"').to_owned();
	Ok(token)
}
/*
Take a look here for reference:
https://docs.getxray.app/space/XRAYCLOUD/44577176/Import+Execution+Results+-+REST+v2#JUnit-XML-Results-Multipart
 */
pub async fn upload_junit_to_xray_cloud(
	client: &Client,
	token: &str,
	junit_path: &Path,
	summary: String,
	test_plan_key: Option<String>,
	environments: Vec<String>,
	labels: Vec<String>,
) -> Result<()> {
	let xml_bytes = read(junit_path).await?;

	let url =
		"https://eu.xray.cloud.getxray.app/api/v2/import/execution/junit/multipart?projectKey=PM";

	let file_name = junit_path
		.file_name()
		.and_then(|s| s.to_str())
		.unwrap_or("junit.xml")
		.to_string();

	let results_part = Part::bytes(xml_bytes).file_name(file_name).mime_str("application/xml")?;

	let mut info_root = Map::new();
	let mut info_fields = Map::new();

	info_fields.insert("project".to_string(), json!({"key": "PM"}));
	info_fields.insert("issuetype".to_string(), json!({"id": "10022"}));
	info_fields.insert("summary".to_string(), json!(summary));
	if !labels.is_empty() {
		info_fields.insert("labels".to_string(), json!(labels));
	}

	info_root.insert("fields".to_string(), Value::Object(info_fields));

	let mut xray_fields = Map::new();
	if let Some(test_plan_key) = test_plan_key {
		xray_fields.insert("testPlanKey".to_string(), json!(test_plan_key));
	}
	if !environments.is_empty() {
		xray_fields.insert("environments".to_string(), json!(environments));
	}

	if !xray_fields.is_empty() {
		info_root.insert("xrayFields".to_string(), Value::Object(xray_fields));
	}

	let info_json = Value::Object(info_root);
	let info_str = info_json.to_string();

	let info_part = Part::bytes(info_str.into_bytes())
		.file_name("info.json")
		.mime_str("application/json")?;

	let form = Form::new().part("results", results_part).part("info", info_part);

	let resp = client
		.post(url)
		.header("Authorization", format!("Bearer {}", token))
		.multipart(form)
		.send()
		.await?;

	let status = resp.status();
	let body = resp.text().await.unwrap_or_default();

	if !status.is_success() {
		anyhow::bail!("Xray JUnit import failed: {} - {}", status, body);
	}

	println!("Xray import OK: {}", body);
	Ok(())
}
