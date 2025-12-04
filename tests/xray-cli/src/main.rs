extern crate core;

mod commands;

use crate::commands::create_test_execution::{
	CreateTestExecutionArgs, upload_junit_to_xray_cloud, xray_cloud_auth_token,
};
use crate::commands::enrich_junit::{EnrichJunitArgs, enrich_junit};
use anyhow::Result;
use clap::{Parser, Subcommand};
use reqwest::Client;
use std::path::Path;
use std::{fs::File, io::BufReader};
use xmltree::Element;

#[derive(Parser)]
#[command(
	name = "xray-cli",
	version,
	about = "XRay Cli tool",
	long_about = "A simple CLI written in Rust to help with XRay integration in Jira"
)]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	/// Enrich a JUnit XML file with additional data needed for Xray import
	EnrichJunit(EnrichJunitArgs),

	/// Create Test Execution out of enriched test results in the Xray server
	CreateTestExecution(CreateTestExecutionArgs),
}

async fn run(command: Commands) -> Result<()> {
	match command {
		Commands::EnrichJunit(EnrichJunitArgs { input, output }) => {
			println!("Running enrich-junit with input:{input} and output:{output}");
			let file = File::open(input)?;
			let root: Element = Element::parse(BufReader::new(file))?;

			let enriched = enrich_junit(root)?;

			let out_file = File::create(output)?;
			enriched.write(out_file)?;
			println!("Successfully enriched and saved!");
		},

		Commands::CreateTestExecution(CreateTestExecutionArgs {
			input,
			name,
			test_plan_key,
			environments,
			labels,
		}) => {
			println!("Upload results from input: {input}...");
			let client = Client::new();

			let client_id = std::env::var("XRAY_CLIENT_ID")?;
			let client_secret = std::env::var("XRAY_CLIENT_SECRET")?;

			let token = xray_cloud_auth_token(&client, client_id, client_secret).await?;

			upload_junit_to_xray_cloud(
				&client,
				&token,
				Path::new(&input),
				name,
				test_plan_key,
				environments,
				labels,
			)
			.await?;
		},
	}
	Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
	let cli = Cli::parse();
	run(cli.command).await
}
