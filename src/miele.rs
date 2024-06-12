use std::fs::File;
use std::io::Write;
use std::path::Path;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use office::{DataType, Excel};
use reqwest::Client;
use urlencoding::decode;

use super::AvailabilityRequest;

///
/// # Miele Availability
/// Gets the availability of the Miele appliances.
///
/// ## Inputs
/// * `request`: `AvailabilityRequest`
///
/// ## Outputs
/// String - The availability of the Miele appliances.
///
/// # Errors
/// todo
#[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
pub async fn miele_availability(req: AvailabilityRequest) -> Result<String, String> {
	let file_name = "miele_appliance_availability.xlsx";
	let root_path = Path::new("/easfiles/appliances/data/");
	let file_path = Path::join(root_path, file_name);

	let client = Client::new();
	let response = match client.get("https://ws15.mieleusa.com/sbo-reports/reports/download.php?id=SlyUOJt9vOFlwUcXZleX").send().await {
		Ok(response) => response,
		Err(e) => {
			return Ok(format!("Failed to get Miele appliance availability spreadsheet: {e:?}"));
		}
	};

	let mut file = match File::create(&file_path) {
		Ok(file) => file,
		Err(e) => {
			return Ok(format!("Failed to create Miele appliance availability spreadsheet: {e:?}"));
		}
	};
	let response_bytes = match response.bytes().await {
		Ok(response_bytes) => response_bytes,
		Err(e) => {
			return Ok(format!("Failed to get Miele appliance availability spreadsheet: {e:?}"));
		}
	};
	match file.write_all(&response_bytes) {
		Ok(()) => (),
		Err(e) => {
			return Ok(format!("Failed to write Miele appliance availability spreadsheet to file: {e:?}"));
		}
	};

	let mut excel = match Excel::open(&file_path) {
		Ok(excel) => excel,
		Err(e) => {
			return Ok(format!("Failed to open Miele appliance availability spreadsheet: {e:?}"));
		}
	};

	let Some(warehouse) = req.warehouse.clone() else { return Ok("No warehouse found.".to_string()) };
	let Some(model_number) = req.model_number.clone() else { return Ok("No model number found.".to_string()) };

	match excel.worksheet_range(&warehouse) {
		Ok(range) => {
			let mut headers: Vec<(String, usize)> = Vec::new();
			let mut i: usize = 0;

			match range.rows().next() {
				Some(row) => {
					for cell in row {
						let value = match cell {
							DataType::String(s) => s.to_string(),
							DataType::Float(f) => f.to_string(),
							DataType::Int(i) => i.to_string(),
							DataType::Bool(b) => b.to_string(),
							_ => String::new(),
						};
						headers.push((value, i));
						i += 1;
					}
				}
				None => return Ok("Failed to get row from Miele appliance availability spreadsheet.".to_string()),
			}

			let mut miele_appliances: Vec<MieleAppliance> = range
				.rows()
				.skip(1)
				.map(|row| {
					let mut appliance = MieleAppliance {
						timestamp: String::new(),
						sku: String::new(),
						upc: String::new(),
						category: String::new(),
						subcategory: String::new(),
						model_number: String::new(),
						description: String::new(),
						current_umrp: String::new(),
						new_umrp: String::new(),
						dealer_cost_level: String::new(),
						warehouse_number: String::new(),
						available_qty: String::new(),
						sales_status: String::new(),
						next_available_qty: String::new(),
						next_available_date: String::new(),
						score: 0.0,
					};

					row.iter().enumerate().for_each(|(i, cell)| {
						let value = match cell {
							DataType::String(s) => s.to_string(),
							DataType::Float(f) => f.to_string(),
							DataType::Int(i) => i.to_string(),
							DataType::Bool(b) => b.to_string(),
							_ => String::new(),
						};
						let header = headers.iter().find(|header| header.1 == i).map_or_else(String::new, |header| header.0.clone());
						match header.as_str().to_lowercase().as_str() {
							"timestamp" => appliance.timestamp = value,
							"sku#" => appliance.sku = value,
							"ean/upc" => appliance.upc = value,
							"category" => appliance.category = value,
							"subcategory" => appliance.subcategory = value,
							"model number" => appliance.model_number = value,
							"description" => appliance.description = value,
							"current umrp/map" => appliance.current_umrp = value,
							"new umrp/map" => appliance.new_umrp = value,
							"dealer cost level" => appliance.dealer_cost_level = value,
							"warehouse no" => appliance.warehouse_number = value,
							"available qty" => appliance.available_qty = value,
							"sales status" => appliance.sales_status = value,
							"next available qty" => appliance.next_available_qty = value,
							"next available date" => appliance.next_available_date = value,
							_ => {}
						}
					});
					appliance
				})
				.collect();

			let mut best_match = MieleAppliance {
				timestamp: String::new(),
				sku: String::new(),
				upc: String::new(),
				category: String::new(),
				subcategory: String::new(),
				model_number: String::new(),
				description: String::new(),
				current_umrp: String::new(),
				new_umrp: String::new(),
				dealer_cost_level: String::new(),
				warehouse_number: String::new(),
				available_qty: String::new(),
				sales_status: String::new(),
				next_available_qty: String::new(),
				next_available_date: String::new(),
				score: 0.0,
			};

			for (i, miele_appliance) in miele_appliances.clone().iter().enumerate() {
				let matcher = SkimMatcherV2::default();

				let app_m_n: String = miele_appliance.model_number.to_lowercase().trim().to_string().chars().filter(|c| !c.is_whitespace()).collect();
				let app_desc: String = miele_appliance.description.to_lowercase().trim().to_string().chars().filter(|c| !c.is_whitespace()).collect();
				let m_n: String = match decode(&model_number) {
					Ok(m_n) => m_n.to_lowercase().trim().to_string().chars().filter(|c| !c.is_whitespace()).collect(),
					Err(_) => {
						return Ok("Cannot decode model number.".to_string());
					}
				};

				let model_number_result = matcher.fuzzy_match(app_m_n.as_str(), m_n.as_str());
				let model_number_score: f64 = model_number_result.map_or(0.0, |model_number_result| model_number_result as f64);

				let description_result = matcher.fuzzy_match(app_desc.as_str(), m_n.as_str());
				let description_score: f64 = description_result.map_or(0.0, |description_result| description_result as f64);

				let score = model_number_score + description_score;
				miele_appliances[i].score = score;
				if score > best_match.score {
					best_match = miele_appliances[i].clone();
				}
			}

			match best_match.next_available_date.as_str() {
				"" => Ok(format!("Next avalability for {} is unknown.", best_match.model_number)),
				_ => Ok(format!("Found: {}, Available: {}", best_match.model_number, best_match.next_available_date)),
			}
		}
		Err(err) => Ok(format!("Error: {err}")),
	}
}

///
/// # Miele Appliance
/// Struct to hold the data from the Miele Excel file.
///
#[derive(Debug, Clone)]
struct MieleAppliance {
	timestamp: String,
	sku: String,
	upc: String,
	category: String,
	subcategory: String,
	model_number: String,
	description: String,
	current_umrp: String,
	new_umrp: String,
	dealer_cost_level: String,
	warehouse_number: String,
	available_qty: String,
	sales_status: String,
	next_available_qty: String,
	next_available_date: String,
	score: f64,
}
