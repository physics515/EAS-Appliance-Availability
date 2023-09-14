use std::fs::File;
use std::io::Write;

use chrono::Local;
use eggersmann_app_server_auth::BSHJWTTokenClaims;
use playwright::Playwright;
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::{Body, Client};
use serde_json::{json, Value};

use super::AvailabilityRequest;

///
/// # BSH Availability
/// Gets the availability of the BSH appliances.
///
/// ## Inputs
/// * `request`: AvailabilityRequest
///
/// ## Outputs
/// String - The availability of the BSH appliances.
///
pub async fn bsh_availability(req: AvailabilityRequest, username: String, password: String) -> String {
	let token = get_bsh_token().await;
	let token = match token {
		Ok(token) => token,
		Err(_) => {
			bsh_login(username, password).await;
			match get_bsh_token().await {
				Ok(token) => token,
				Err(e) => {
					panic!("Faild to login to BSH website: {e:?}");
				}
			}
		}
	};

	let cookies: String = {
		let mut cookies: String = String::new();
		for cookie in token.bsh_cookies.iter() {
			cookies.push_str(&cookie.name);
			cookies.push('=');
			cookies.push_str(&cookie.value);
			cookies.push_str("; ");
		}
		cookies
	};

	//get x_csrf_token
	let client = Client::new();
	let mut headers = HeaderMap::new();

	// Set cookie in headers
	match HeaderValue::from_str(&cookies) {
		Ok(cookie) => headers.insert(header::COOKIE, cookie),
		Err(e) => panic!("Failed to create cookie header: {e:?}"),
	};

	// Set x-csrf-token in headers
	match HeaderValue::from_str(" Fetch") {
		Ok(x_csrf_token) => headers.insert("x-csrf-token", x_csrf_token),
		Err(e) => panic!("Failed to create x_csrf_token header: {e:?}"),
	};
	let today = Local::now().format("%Y%m%d").to_string();
	let x_csrf_token: String = {
		let resp = match client.get("https://b2bportal-cloud.bsh-partner.com/sap/opu/odata/bshb2b/SD_OM_SRV/").headers(headers).send().await {
			Ok(resp) => resp,
			Err(e) => panic!("Failed to get x_csrf_token: {e:?}"),
		};
		match resp.headers().get("x-csrf-token") {
			Some(x_csrf_token) => match x_csrf_token.to_str() {
				Ok(x_csrf_token) => x_csrf_token.to_string(),
				Err(e) => panic!("Failed to convert x_csrf_token to string: {e:?}"),
			},
			None => panic!("Failed to get x_csrf_token"),
		}
	};

	//get availability
	let data = json!({
		"Country": "US",
		"Brand": "A00",
		"Submodule": "APPS",
		"DocCategory": "ASTD",
		"PurchNo": "",
		"ReqDateH": today,
		"ComplDlv": "",
		"SoldTo": "5010011875",
		"Language": "EN",
		"ShipTo": req.warehouse.clone(),
		"SOSimulateToItem": [
			{
				"Submodule": "APPS",
				"Material": req.model_number.clone(),
				"ReqQty": "1",
				"ReqDateI": today
			}
		]
	})
	.to_string();

	headers = HeaderMap::new();

	// Set cookie in headers
	match HeaderValue::from_str(&cookies.clone()) {
		Ok(cookie) => headers.insert(header::COOKIE, cookie),
		Err(e) => panic!("Failed to create cookie header: {e:?}"),
	};

	// Set x-csrf-token in headers
	match HeaderValue::from_str(&x_csrf_token) {
		Ok(x_csrf_token) => headers.insert("x-csrf-token", x_csrf_token),
		Err(e) => panic!("Failed to create x_csrf_token header: {e:?}"),
	};

	// Set content-type in headers
	match HeaderValue::from_str("application/json") {
		Ok(content_type) => headers.insert(header::CONTENT_TYPE, content_type),
		Err(e) => panic!("Failed to create content-type header: {e:?}"),
	};

	// Set accept in headers
	match HeaderValue::from_str("application/json") {
		Ok(accept) => headers.insert(header::ACCEPT, accept),
		Err(e) => panic!("Failed to create accept header: {e:?}"),
	};

	// Set data in headers
	match HeaderValue::from_str(&data) {
		Ok(data) => headers.insert("data", data),
		Err(e) => panic!("Failed to create data header: {e:?}"),
	};

	let response = match client.post("https://b2bportal-cloud.bsh-partner.com/sap/opu/odata/bshb2b/SD_OM_SRV/SOSimulate").headers(headers).body(Body::from(data)).send().await {
		Ok(response) => response,
		Err(e) => panic!("Failed to get availability response: {e:?}"),
	};
	let response_text = match response.text().await {
		Ok(response_text) => response_text,
		Err(e) => panic!("Failed to get availability response text: {e:?}"),
	};
	let response_data: serde_json::Value = match serde_json::from_str(&response_text) {
		Ok(response_data) => response_data,
		Err(e) => panic!("Failed to parse availability response text: {e:?}"),
	};
	let mut availability = response_data["d"]["SOSimulateToItem"]["results"][0]["AvailBackorder"].to_string();

	if availability.contains("\\n") || availability.contains('\n') {
		availability = availability.replace("\\n", "");
		availability = availability.replace('\n', "");
	}

	if availability.contains("\\r") || availability.contains('\r') {
		availability = availability.replace("\\r", "");
		availability = availability.replace('\r', "");
	}

	if availability.contains(' ') {
		availability = availability.replace(' ', "");
	}

	match availability.len() < 10 {
		true => availability = "Model availablility not found.".to_string(),
		false => availability = availability.to_string(),
	}

	availability
}

///
/// Gets the BSHJWTToken from the the server storage.
///
async fn get_bsh_token() -> Result<BSHJWTTokenClaims, String> {
	let file = match File::open("/easfiles/appliances/cookies/bsh_cookies.json") {
		Ok(file) => file,
		Err(e) => return Err(format!("Failed to open bsh_cookies.json: {e:?}")),
	};
	let file: Value = match serde_json::from_reader(file) {
		Ok(file) => file,
		Err(e) => return Err(format!("Failed to parse bsh_cookies.json: {e:?}")),
	};
	let token = match file["token"].as_str() {
		Some(token) => token,
		None => return Err("Failed to get token from bsh_cookies.json".to_string()),
	};
	BSHJWTTokenClaims::decode(token).await
}

///
/// # Login to BSH System
///
/// ## Outputs
/// bool - True if login was successful, false otherwise.
///
pub async fn bsh_login(username: String, password: String) -> bool {
	let playwright = Playwright::initialize().await.unwrap();
	playwright.prepare().unwrap();

	let chromium = playwright.chromium();
	let browser = chromium.launcher().headless(true).launch().await.unwrap();
	let context = browser.context_builder().build().await.unwrap();
	let page = context.new_page().await.unwrap();

	page.goto_builder("https://b2bportal.bsh-partner.com").goto().await.unwrap();
	page.fill_builder("input#username", &username).fill().await.unwrap();
	page.fill_builder("#password", &password).fill().await.unwrap();
	page.click_builder("body > div > div > section > div:nth-child(2) > div > form > div:nth-child(3) > div.small-12.medium-4.columns > button").click().await.unwrap();
	page.focus("#SD_OM-BDI-content", None).await.unwrap();

	let url = page.url().unwrap();

	if let Ok(cookies) = context.cookies(&[url]).await {
		let token_json = json!({ "token": BSHJWTTokenClaims::encode(cookies).await.unwrap() }).to_string();
		let mut file = File::create("/easfiles/appliances/cookies/bsh_cookies.json").unwrap();
		file.write_all(token_json.as_bytes()).unwrap();
		true
	} else {
		false
	}
}
