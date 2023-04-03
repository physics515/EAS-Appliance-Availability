use super::AvailabilityRequest;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use office::{DataType, Excel};
use reqwest::Client;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use urlencoding::decode;

///
/// # Miele Availability
/// Gets the availability of the Miele appliances.
///
/// ## Inputs
/// * `request`: AvailabilityRequest
///
/// ## Outputs
/// String - The availability of the Miele appliances.
///
pub async fn miele_availability(req: AvailabilityRequest) -> String {
	let file_name = "miele_appliance_availability.xlsx";
	let root_path = Path::new("/easfiles/appliances/data/");
	let file_path = Path::join(root_path, file_name);

	let client = Client::new();
	let response = match client.get("https://ws15.mieleusa.com/sbo-reports/reports/download.php?id=SlyUOJt9vOFlwUcXZleX").send().await {
		Ok(response) => response,
		Err(e) => {
			panic!("Failed to get Miele appliance availability spreadsheet: {e:?}");
		}
	};

	let mut file = match File::create(&file_path) {
		Ok(file) => file,
		Err(e) => {
			panic!("Failed to create Miele appliance availability spreadsheet: {e:?}");
		}
	};
	let response_bytes = match response.bytes().await {
		Ok(response_bytes) => response_bytes,
		Err(e) => {
			panic!("Failed to get Miele appliance availability spreadsheet: {e:?}");
		}
	};
	match file.write_all(&response_bytes) {
		Ok(_) => (),
		Err(e) => {
			panic!("Failed to write Miele appliance availability spreadsheet to file: {e:?}");
		}
	};

	let mut excel = match Excel::open(&file_path) {
		Ok(excel) => excel,
		Err(e) => {
			panic!("Failed to open Miele appliance availability spreadsheet: {e:?}");
		}
	};

	let warehouse = match req.warehouse.clone() {
		Some(warehouse) => warehouse,
		None => return "No warehouse found.".to_string(),
	};

	let model_number = match req.model_number.clone() {
		Some(model_number) => model_number,
		None => return "No model number found.".to_string(),
	};

	match excel.worksheet_range(&warehouse) {
		Ok(range) => {
			let mut headers: Vec<(String, usize)> = Vec::new();
			let mut i: usize = 0;

			match range.rows().next() {
				Some(row) => {
					row.iter().for_each(|cell| {
						let value = match cell {
							DataType::String(s) => s.to_string(),
							DataType::Float(f) => f.to_string(),
							DataType::Int(i) => i.to_string(),
							DataType::Bool(b) => b.to_string(),
							_ => "".to_string(),
						};
						headers.push((value, i));
						i += 1;
					});
				}
				None => return "Failed to get row from Miele appliance availability spreadsheet.".to_string(),
			}

			let mut miele_appliances: Vec<MieleAppliance> = range
				.rows()
				.skip(1)
				.map(|row| {
					let mut appliance = MieleAppliance {
						timestamp: "".to_string(),
						sku: "".to_string(),
						upc: "".to_string(),
						category: "".to_string(),
						subcategory: "".to_string(),
						model_number: "".to_string(),
						description: "".to_string(),
						current_umrp: "".to_string(),
						new_umrp: "".to_string(),
						dealer_cost_level: "".to_string(),
						warehouse_number: "".to_string(),
						available_qty: "".to_string(),
						sales_status: "".to_string(),
						next_available_qty: "".to_string(),
						next_available_date: "".to_string(),
						score: 0.0,
					};

					row.iter().enumerate().for_each(|(i, cell)| {
						let value = match cell {
							DataType::String(s) => s.to_string(),
							DataType::Float(f) => f.to_string(),
							DataType::Int(i) => i.to_string(),
							DataType::Bool(b) => b.to_string(),
							_ => "".to_string(),
						};
						let header = match headers.iter().find(|header| header.1 == i) {
							Some(header) => header.0.clone(),
							None => "".to_string(),
						};
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
				timestamp: "".to_string(),
				sku: "".to_string(),
				upc: "".to_string(),
				category: "".to_string(),
				subcategory: "".to_string(),
				model_number: "".to_string(),
				description: "".to_string(),
				current_umrp: "".to_string(),
				new_umrp: "".to_string(),
				dealer_cost_level: "".to_string(),
				warehouse_number: "".to_string(),
				available_qty: "".to_string(),
				sales_status: "".to_string(),
				next_available_qty: "".to_string(),
				next_available_date: "".to_string(),
				score: 0.0,
			};

			for (i, miele_appliance) in miele_appliances.clone().iter().enumerate() {
				let matcher = SkimMatcherV2::default();

				let app_m_n: String = miele_appliance.model_number.to_lowercase().trim().to_string().chars().filter(|c| !c.is_whitespace()).collect();
				let app_desc: String = miele_appliance.description.to_lowercase().trim().to_string().chars().filter(|c| !c.is_whitespace()).collect();
				let m_n: String = decode(&model_number).expect("Cannot decode model number.").to_lowercase().trim().to_string().chars().filter(|c| !c.is_whitespace()).collect();

				let model_number_result = matcher.fuzzy_match(app_m_n.as_str(), m_n.as_str());
				let model_number_score: f32 = match model_number_result {
					Some(model_number_result) => model_number_result as f32,
					None => 0.0,
				};
				//println!("app model number: {}, model number: {}, score: {}", app_m_n, m_n, model_number_score);

				let description_result = matcher.fuzzy_match(app_desc.as_str(), m_n.as_str());
				let description_score: f32 = match description_result {
					Some(description_result) => description_result as f32,
					None => 0.0,
				};

				let score = model_number_score + description_score;
				miele_appliances[i].score = score;
				if score > best_match.score {
					best_match = miele_appliances[i].clone();
				}
			}

			match best_match.next_available_date.as_str() {
				"" => {
					format!("Next avalability for {} is unknown.", best_match.model_number)
				}
				_ => {
					format!("Found: {}, Available: {}", best_match.model_number, best_match.next_available_date)
				}
			}
		}
		Err(err) => {
			format!("Error: {err}")
		}
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
	score: f32,
}
