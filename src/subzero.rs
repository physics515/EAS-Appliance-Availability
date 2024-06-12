use std::fs::File;
use std::io::Write;

use chrono::DateTime;
use chrono::Utc;
use duration_string::DurationString;
use eggersmann_app_server_auth::SubZeroJWTTokenClaims;
use playwright::api::Cookie as PlaywrightCookie;
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::Body;
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::{json, Value};

use super::AvailabilityRequest;

///
/// # `SubZero` Availability
/// Gets the availability of the `SubZero` appliances.
///
/// ## Inputs
/// * `req`: `AvailabilityRequest`
///
/// ## Outputs
/// String - The availability of the `SubZero` appliances.
///
/// # Errors
/// todo
pub async fn subzero_availability(mut req: AvailabilityRequest, username: String, password: String) -> Result<String, String> {
	// get subzero token, if not already obtained then login.
	let token = get_subzero_token().await;
	let token = if let Ok(token) = token {
		token
	} else {
		subzero_login(username, password).await?;
		match get_subzero_token().await {
			Ok(token) => token,
			Err(e) => return Ok(format!("Failed to get SubZero token: {e:?}")),
		}
	};

	// parse cookies from token
	let cookies: String = {
		let mut cookies: String = String::new();
		for cookie in &token.subzero_cookies {
			cookies.push_str(&cookie.name);
			cookies.push('=');
			cookies.push_str(&cookie.value);
			cookies.push_str("; ");
		}
		cookies
	};

	// get the number of items in the SubZero cart, if it contains items then clear the cart.
	let mut number_of_items = subzero_get_number_of_items(&cookies).await;
	while number_of_items > 0 {
		subzero_remove_item(&cookies).await;
		number_of_items = subzero_get_number_of_items(&cookies).await;
	}

	// validate the requested model number is in the SubZero catalog.
	req.model_number = match &req.model_number {
		Some(model_number) => Some(subzero_validate_model_number(model_number.to_string(), &cookies).await),
		None => return Ok("No model number provided".to_string()),
	};

	// add items to the SubZero cart and return availability.
	match &req.model_number {
		Some(model_number) => Ok(subzero_add_item(model_number.to_string(), &cookies).await),
		None => Ok("Model number not provided.".to_string()),
	}
}

///
/// # Get `SubZero` Token
/// Retrives the `SubZero` token from the server.
///
/// ## Outputs
/// Result<`SubZeroJWTTokenClaims`, String> - The `SubZero` token claims.
///
async fn get_subzero_token() -> Result<SubZeroJWTTokenClaims, String> {
	let file = match File::open("/easfiles/appliances/cookies/subzero_cookies.json") {
		Ok(file) => file,
		Err(e) => return Err(format!("Failed to open SubZero token file: {e:?}")),
	};
	let file: Value = match serde_json::from_reader(file) {
		Ok(file) => file,
		Err(e) => return Err(format!("Failed to parse SubZero token file: {e:?}")),
	};
	let Some(token) = file["token"].as_str() else { return Err("Failed to get SubZero token from file.".to_string()) };
	SubZeroJWTTokenClaims::decode(token).await
}

///
/// # Get the Number of Items in the `SubZero` Cart
/// Gets the number of items in the `SubZero` cart.
///
/// ## Inputs
/// * `cookies`: String - The cookies to use for the request.
///
/// ## Outputs
/// u32 - The number of items in the `SubZero` cart.
///
async fn subzero_get_number_of_items(cookies: &str) -> u32 {
	let client = Client::new();
	let data = json!({
		"mode": " view",
		"error": " 0",
	})
	.to_string();

	let mut headers = HeaderMap::new();
	match HeaderValue::from_str(cookies) {
		Ok(cookies) => headers.insert(header::COOKIE, cookies),
		Err(_) => return 0,
	};
	match HeaderValue::from_str(" Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/99.0.4844.51 Safari/537.36 Edg/99.0.1150.30") {
		Ok(user_agent) => headers.insert(header::USER_AGENT, user_agent),
		Err(_) => return 0,
	};
	match HeaderValue::from_str("application/x-www-form-urlencoded") {
		Ok(content_type) => headers.insert(header::CONTENT_TYPE, content_type),
		Err(_) => return 0,
	};
	match HeaderValue::from_str(data.as_str()) {
		Ok(data) => headers.insert("data", data),
		Err(_) => return 0,
	};

	let Ok(response) = client.get("https://order.subzero.com/instance1/servlet/WebDispatcher?mode=view&error=0").headers(headers).body(Body::from(data)).send().await else { return 0 };
	let Ok(response_data) = response.text().await else { return 0 };
	let document = Html::parse_document(&response_data);
	let Ok(tr_selector) = Selector::parse("tr") else { return 0 };
	let Ok(my_scroll_table_selector) = Selector::parse("#myScrollTable") else { return 0 };
	let my_scroll_table = document.select(&my_scroll_table_selector).next();
	my_scroll_table.map_or(0, |my_scroll_table| {
		let my_scroll_table_rows = my_scroll_table.select(&tr_selector);
		u32::try_from(my_scroll_table_rows.into_iter().count()).unwrap_or(0)
	})
}

///
/// # Remove Item
/// Removes the first item from the `SubZero` cart.
///
/// ## Inputs
/// * `cookies`: String - The cookies to use for the request.
///
async fn subzero_remove_item(cookies: &str) {
	let client = Client::new();

	let mut headers = HeaderMap::new();
	match HeaderValue::from_str(cookies) {
		Ok(cookies) => headers.insert(header::COOKIE, cookies),
		Err(_) => return,
	};

	match HeaderValue::from_str(" Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/99.0.4844.51 Safari/537.36 Edg/99.0.1150.30") {
		Ok(user_agent) => headers.insert(header::USER_AGENT, user_agent),
		Err(_) => return,
	};

	match HeaderValue::from_str("application/x-www-form-urlencoded") {
		Ok(content_type) => headers.insert(header::CONTENT_TYPE, content_type),
		Err(_) => return,
	};

	let params = [("mode", "delete"), ("index", "0"), ("x", "3"), ("y", "9")];
	match client.post("https://order.subzero.com/instance1/servlet/WebDispatcher?mode=delete&index=0&x=3&y=9").headers(headers).form(&params).send().await {
		Ok(_) => (),
		Err(e) => panic!("Failed to remove item from cart: {e:?}"),
	};
}

///
/// # Add Item
/// Adds an item to the `SubZero` cart and returns the availablility date.
///
/// ## Inputs
/// * `cookies`: String - The cookies to use for the request.
/// * `model_number`: String - The model number of the item to add.
///
/// ## Outputs
/// String - The availability date of the item.
///
async fn subzero_add_item(model_number: String, cookies: &str) -> String {
	let client = Client::new();

	let mut headers = HeaderMap::new();
	match HeaderValue::from_str(cookies) {
		Ok(cookies) => headers.insert(header::COOKIE, cookies),
		Err(e) => return format!("Faild to add cookies to header: {e:?}"),
	};
	match HeaderValue::from_str(" Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/99.0.4844.51 Safari/537.36 Edg/99.0.1150.30") {
		Ok(user_agent) => headers.insert(header::USER_AGENT, user_agent),
		Err(e) => return format!("Failed to add user agent to header: {e:?}"),
	};
	match HeaderValue::from_str("application/x-www-form-urlencoded") {
		Ok(content_type) => headers.insert(header::CONTENT_TYPE, content_type),
		Err(e) => return format!("Failed to add content type to header: {e:?}"),
	};
	let params = [("item", &model_number), ("quantity", &"1".to_string())];

	let data = json!({
		"item": model_number,
		"quantity": "1",
	});

	let response = match client.post("https://order.subzero.com/instance1/servlet/WebDispatcher?mode=add").headers(headers).body(Body::from(data.to_string())).form(&params).send().await {
		Ok(response) => response,
		Err(e) => return format!("Failed to add item to cart: {e:?}"),
	};

	let response_data = match response.text().await {
		Ok(response_data) => response_data,
		Err(e) => return format!("Failed to get response data: {e:?}"),
	};

	let document = Html::parse_document(&response_data);
	let my_scroll_table_selector = match Selector::parse("#myScrollTable") {
		Ok(my_scroll_table_selector) => my_scroll_table_selector,
		Err(e) => return format!("Failed to parse my scroll table selector: {e:?}"),
	};
	let table_body_selector = match Selector::parse("tbody") {
		Ok(table_body_selector) => table_body_selector,
		Err(e) => return format!("Failed to parse table body selector: {e:?}"),
	};
	let row_selector = match Selector::parse("tr") {
		Ok(row_selector) => row_selector,
		Err(e) => return format!("Failed to parse row selector: {e:?}"),
	};
	let td_selector = match Selector::parse("td") {
		Ok(td_selector) => td_selector,
		Err(e) => return format!("Failed to parse td selector: {e:?}"),
	};

	let my_scroll_table = document.select(&my_scroll_table_selector).next();
	my_scroll_table.map_or_else(
		|| "Error finding item.".to_string(),
		|my_scroll_table| {
			let table_body = my_scroll_table.select(&table_body_selector).next();
			table_body.map_or_else(
				|| "Error finding item.".to_string(),
				|table_body| {
					let mut availability: String = "Error finding item.".to_string();
					let rows = table_body.select(&row_selector);
					for row in rows {
						let cells = row.select(&td_selector);
						for (i, cell) in cells.enumerate() {
							if i == 7 {
								availability = cell.inner_html().to_string();
							}
						}
					}
					availability
				},
			)
		},
	)
}

async fn subzero_validate_model_number(model_number: String, cookies: &str) -> String {
	let client = Client::new();
	let mut headers = HeaderMap::new();

	match HeaderValue::from_str("*/*") {
		Ok(accept) => headers.insert(header::ACCEPT, accept),
		Err(e) => return format!("Failed to add accept to header: {e:?}"),
	};

	// add user agent to header
	match HeaderValue::from_str("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/99.0.4844.51 Safari/537.36 Edg/99.0.1150.30") {
		Ok(user_agent) => headers.insert(header::USER_AGENT, user_agent),
		Err(e) => return format!("Failed to add user agent to header: {e:?}"),
	};

	match HeaderValue::from_str(cookies) {
		Ok(cookies) => headers.insert(header::COOKIE, cookies),
		Err(e) => return format!("Faild to add cookies to header: {e:?}"),
	};

	// add host to header
	match HeaderValue::from_str("order.subzero.com") {
		Ok(host) => headers.insert(header::HOST, host),
		Err(e) => return format!("Failed to add host to header: {e:?}"),
	};

	let url = format!("https://order.subzero.com/instance1/servlet/WebDispatcher?mode=suggest&type=advanced&search={model_number}");

	let response = match client.get(url).headers(headers).send().await {
		Ok(response) => response,
		Err(e) => return format!("Failed to get suggested items: {e:?}"),
	};

	let response_data = match response.text().await {
		Ok(response_data) => response_data,
		Err(e) => return format!("Failed to get suggested items: {e:?}"),
	};

	let response_data = response_data.split('{').collect::<Vec<&str>>()[0].to_string();
	response_data
}

///
/// # Login to `SubZero` System
///
/// ## Outputs
/// bool - True if login was successful, false otherwise.
///
/// # Errors
/// todo
pub async fn subzero_login(username: String, password: String) -> Result<(), String> {
	let mut headers = HeaderMap::new();
	headers.insert(header::USER_AGENT, " Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/99.0.4844.51 Safari/537.36 Edg/99.0.1150.30".parse().map_err(|e| format!("Failed to add user agent to header: {e:?}"))?);
	headers.insert(header::ACCESS_CONTROL_ALLOW_CREDENTIALS, "true".parse().map_err(|e| format!("Failed to add access control allow credentials to header: {e:?}"))?);

	let response = reqwest::Client::new().post("https://order.subzero.com/instance1/servlet/WebDispatcher").headers(headers).form(&[("user", username.as_str()), ("psswd", password.as_str()), ("mode", "logon"), ("env", "EnvZZ")]).send().await.map_err(|e| format!("Failed to send login request: {e:?}"))?;

	// get response cookies into json
	let mut cookies_json_vec: Vec<serde_json::Value> = Vec::new();
	let mut subzero_cookies: Vec<PlaywrightCookie> = Vec::new();

	for cookie in response.cookies() {
		let name = cookie.name().to_string();
		let value = cookie.value().to_string();
		let expires = cookie.expires().map_or_else(String::new, |expires| {
			let time: DateTime<Utc> = expires.into();
			time.to_string()
		});
		let max_age = cookie.max_age().map_or_else(String::new, |max_age| DurationString::from(max_age).into());
		let domain = cookie.domain().map_or_else(String::new, std::string::ToString::to_string);
		let path = cookie.path().map_or_else(String::new, std::string::ToString::to_string);

		cookies_json_vec.push(json!({
			"name": name,
			"value": value,
			"expires": expires,
			"max_age": max_age,
			"domain": domain,
			"path": path,
		}));
	}

	for cookie in cookies_json_vec {
		let expires = cookie["expires"].as_str().map(std::string::ToString::to_string).and_then(|expires| expires.parse().map_err(|e| format!("Failed to parse expires: {e:?}")).ok());
		let domain = cookie["domain"].as_str().map(std::string::ToString::to_string);
		let path = cookie["path"].as_str().map(std::string::ToString::to_string);
		let name = cookie["name"].as_str().ok_or_else(|| "Failed to get cookie name".to_string()).map_err(|err| format!("Failed to get cookie name: {err:?}"))?.to_string();
		let value = cookie["value"].as_str().ok_or_else(|| "Failed to get cookie value".to_string()).map_err(|err| format!("Failed to get cookie value: {err:?}"))?.to_string();
		let new_cookie: PlaywrightCookie = PlaywrightCookie { name, value, expires, domain, path, url: None, secure: None, http_only: None, same_site: None };
		subzero_cookies.push(new_cookie);
	}

	if !subzero_cookies.is_empty() {
		let token_json = json!({ "token": SubZeroJWTTokenClaims::encode(subzero_cookies).await.map_err(|e| format!("Error encoding token: {e}"))? }).to_string();
		let mut file = File::create("/easfiles/appliances/cookies/subzero_cookies.json").map_err(|e| format!("Failed to create SubZero token file: {e:?}"))?;
		file.write_all(token_json.as_bytes()).map_err(|e| format!("Failed to write SubZero token file: {e:?}"))?;
	}

	Ok(())
}
