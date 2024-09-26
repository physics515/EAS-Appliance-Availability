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
/// * `request`: `AvailabilityRequest`
///
/// ## Outputs
/// String - The availability of the BSH appliances.
///
/// # Errors
/// todo
#[allow(clippy::too_many_lines)]
pub async fn bsh_availability(req: AvailabilityRequest, username: String, password: String) -> Result<String, String> {
	let token = get_bsh_token().await;
	let token = if let Ok(token) = token {
		token
	} else {
		bsh_login(username, password).await?;
		match get_bsh_token().await {
			Ok(token) => token,
			Err(e) => {
				return Ok(format!("Faild to login to BSH website: {e:?}"));
			}
		}
	};

	let cookies: String = {
		let mut cookies: String = String::new();
		for cookie in &token.bsh_cookies {
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
		Err(e) => return Ok(format!("Failed to create cookie header: {e:?}")),
	};

	// Set x-csrf-token in headers
	match HeaderValue::from_str(" Fetch") {
		Ok(x_csrf_token) => headers.insert("x-csrf-token", x_csrf_token),
		Err(e) => return Ok(format!("Failed to create x_csrf_token header: {e:?}")),
	};
	let today = Local::now().format("%Y%m%d").to_string();
	let x_csrf_token: String = {
		let resp = match client.get("https://b2bportal-cloud.bsh-partner.com/sap/opu/odata/bshb2b/SD_OM_SRV/").headers(headers).send().await {
			Ok(resp) => resp,
			Err(e) => return Ok(format!("Failed to get x_csrf_token: {e:?}")),
		};
		resp.headers().get("x-csrf-token").map_or_else(
			|| Ok("Failed to get x_csrf_token".to_string()),
			|x_csrf_token| match x_csrf_token.to_str() {
				Ok(x_csrf_token) => Ok::<String, String>(x_csrf_token.to_string()),
				Err(e) => Ok(format!("Failed to convert x_csrf_token to string: {e:?}")),
			},
		)?
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
		Err(e) => return Ok(format!("Failed to create cookie header: {e:?}")),
	};

	// Set x-csrf-token in headers
	match HeaderValue::from_str(&x_csrf_token) {
		Ok(x_csrf_token) => headers.insert("x-csrf-token", x_csrf_token),
		Err(e) => return Ok(format!("Failed to create x_csrf_token header: {e:?}")),
	};

	// Set content-type in headers
	match HeaderValue::from_str("application/json") {
		Ok(content_type) => headers.insert(header::CONTENT_TYPE, content_type),
		Err(e) => return Ok(format!("Failed to create content-type header: {e:?}")),
	};

	// Set accept in headers
	match HeaderValue::from_str("application/json") {
		Ok(accept) => headers.insert(header::ACCEPT, accept),
		Err(e) => return Ok(format!("Failed to create accept header: {e:?}")),
	};

	// Set data in headers
	match HeaderValue::from_str(&data) {
		Ok(data) => headers.insert("data", data),
		Err(e) => return Ok(format!("Failed to create data header: {e:?}")),
	};

	let response = match client.post("https://b2bportal-cloud.bsh-partner.com/sap/opu/odata/bshb2b/SD_OM_SRV/SOSimulate").headers(headers).body(Body::from(data)).send().await {
		Ok(response) => response,
		Err(e) => return Ok(format!("Failed to get availability response: {e:?}")),
	};
	let response_text = match response.text().await {
		Ok(response_text) => response_text,
		Err(e) => return Ok(format!("Failed to get availability response text: {e:?}")),
	};
	let response_data: serde_json::Value = match serde_json::from_str(&response_text) {
		Ok(response_data) => response_data,
		Err(e) => return Ok(format!("Failed to parse availability response text: {e:?}")),
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

	if availability.len() < 10 {
		availability = "Model availablility not found.".to_string();
	} else {
		availability = availability.to_string();
	}

	Ok(availability)
}

///
/// Gets the `BSHJWTToken` from the the server storage.
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
	let Some(token) = file["token"].as_str() else { return Err("Failed to get token from bsh_cookies.json".to_string()) };
	BSHJWTTokenClaims::decode(token).await
}

///
/// # Login to BSH System
///
/// ## Outputs
/// bool - True if login was successful, false otherwise.
///
/// # Errors
/// todo
pub async fn bsh_login(username: String, password: String) -> Result<bool, String> {
	let playwright = Playwright::initialize().await.map_err(|e| format!("Failed to initialize playwright: {e:?}"))?;
	playwright.prepare().map_err(|e| format!("Failed to prepare playwright: {e:?}"))?;

	let chromium = playwright.chromium();
	let browser = chromium.launcher().headless(true).launch().await.map_err(|e| format!("Failed to launch chromium: {e:?}"))?;
	let context = browser.context_builder().build().await.map_err(|e| format!("Failed to build context: {e:?}"))?;
	let page = context.new_page().await.map_err(|e| format!("Failed to create new page: {e:?}"))?;

	page.goto_builder("https://b2bportal.bsh-partner.com").goto().await.map_err(|e| format!("Failed to go to BSH website: {e:?}"))?;
	page.fill_builder("input#username", &username).fill().await.map_err(|e| format!("Failed to fill username: {e:?}"))?;
	page.fill_builder("#password", &password).fill().await.map_err(|e| format!("Failed to fill password: {e:?}"))?;
	page.click_builder("body > div > div > section > div:nth-child(2) > div > form > div:nth-child(3) > div.small-12.medium-4.columns > button").click().await.map_err(|e| format!("Failed to click login: {e:?}"))?;
	page.focus("#SD_OM-BDI-content", None).await.map_err(|e| format!("Failed to focus on SD_OM-BDI-content: {e:?}"))?;

	let url = page.url().map_err(|e| format!("Failed to get page url: {e:?}"))?;

	if let Ok(cookies) = context.cookies(&[url]).await {
		let token_json = json!({ "token": BSHJWTTokenClaims::encode(cookies).await.map_err(|_| "Faild to encode BSH Token.".to_string())? }).to_string();
		let mut file = File::create("/easfiles/appliances/cookies/bsh_cookies.json").map_err(|e| format!("Failed to create bsh_cookies.json: {e:?}"))?;
		file.write_all(token_json.as_bytes()).map_err(|e| format!("Failed to write bsh_cookies.json: {e:?}"))?;
		Ok(true)
	} else {
		Ok(false)
	}
}
