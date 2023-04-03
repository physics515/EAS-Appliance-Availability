use super::AvailabilityRequest;
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
use std::fs::File;
use std::io::Write;

///
/// # SubZero Availability
/// Gets the availability of the SubZero appliances.
///
/// ## Inputs
/// * `req`: AvailabilityRequest
///
/// ## Outputs
/// String - The availability of the SubZero appliances.
///
pub async fn subzero_availability(mut req: AvailabilityRequest, username: String, password: String) -> String {
	// get subzero token, if not already obtained then login.
	let token = get_subzero_token().await;
	let token = match token {
		Ok(token) => token,
		Err(_) => {
			subzero_login(username, password).await;
			match get_subzero_token().await {
				Ok(token) => token,
				Err(e) => return format!("Failed to get SubZero token: {e:?}"),
			}
		}
	};

	// parse cookies from token
	let cookies: String = {
		let mut cookies: String = String::new();
		for cookie in token.subzero_cookies.iter() {
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
		None => return "No model number provided".to_string(),
	};

	// add items to the SubZero cart and return availability.
	match &req.model_number {
		Some(model_number) => subzero_add_item(model_number.to_string(), &cookies).await,
		None => "Model number not provided.".to_string(),
	}
}

///
/// # Get SubZero Token
/// Retrives the SubZero token from the server.
///
/// ## Outputs
/// Result<SubZeroJWTTokenClaims, String> - The SubZero token claims.
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
	let token = match file["token"].as_str() {
		Some(token) => token,
		None => return Err("Failed to get SubZero token from file.".to_string()),
	};
	SubZeroJWTTokenClaims::decode(token).await
}

///
/// # Get the Number of Items in the SubZero Cart
/// Gets the number of items in the SubZero cart.
///
/// ## Inputs
/// * `cookies`: String - The cookies to use for the request.
///
/// ## Outputs
/// u32 - The number of items in the SubZero cart.
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
		Err(e) => {
			println!("Faild to add cookies to header: {e:?}");
			return 0;
		}
	};
	match HeaderValue::from_str(" Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/99.0.4844.51 Safari/537.36 Edg/99.0.1150.30") {
		Ok(user_agent) => headers.insert(header::USER_AGENT, user_agent),
		Err(e) => {
			println!("Faild to add user agent to header: {e:?}");
			return 0;
		}
	};
	match HeaderValue::from_str("application/x-www-form-urlencoded") {
		Ok(content_type) => headers.insert(header::CONTENT_TYPE, content_type),
		Err(e) => {
			println!("Faild to add content type to header: {e:?}");
			return 0;
		}
	};
	match HeaderValue::from_str(data.as_str()) {
		Ok(data) => headers.insert("data", data),
		Err(e) => {
			println!("Faild to add data to header: {e:?}");
			return 0;
		}
	};

	let response = match client.get("https://order.subzero.com/instance1/servlet/WebDispatcher?mode=view&error=0").headers(headers).body(Body::from(data)).send().await {
		Ok(response) => response,
		Err(e) => {
			println!("Failed to get number of items in cart response: {e:?}");
			return 0;
		}
	};
	let response_data = match response.text().await {
		Ok(response_data) => response_data,
		Err(e) => {
			println!("Failed to get number of items in cart response data: {e:?}");
			return 0;
		}
	};
	let document = Html::parse_document(&response_data);
	let tr_selector = match Selector::parse("tr") {
		Ok(tr_selector) => tr_selector,
		Err(e) => {
			println!("Failed to get row selector: {e:?}");
			return 0;
		}
	};
	let my_scroll_table_selector = match Selector::parse("#myScrollTable") {
		Ok(my_scroll_table_selector) => my_scroll_table_selector,
		Err(e) => {
			println!("Failed to get scroll table selector: {e:?}");
			return 0;
		}
	};
	let my_scroll_table = document.select(&my_scroll_table_selector).next();
	match my_scroll_table {
		Some(my_scroll_table) => {
			let my_scroll_table_rows = my_scroll_table.select(&tr_selector);
			my_scroll_table_rows.into_iter().count() as u32
		}
		None => 0,
	}
}

///
/// # Remove Item
/// Removes the first item from the SubZero cart.
///
/// ## Inputs
/// * `cookies`: String - The cookies to use for the request.
///
async fn subzero_remove_item(cookies: &str) {
	let client = Client::new();

	let mut headers = HeaderMap::new();
	match HeaderValue::from_str(cookies) {
		Ok(cookies) => headers.insert(header::COOKIE, cookies),
		Err(e) => {
			println!("Faild to add cookies to header: {e:?}");
			return;
		}
	};
	match HeaderValue::from_str(" Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/99.0.4844.51 Safari/537.36 Edg/99.0.1150.30") {
		Ok(user_agent) => headers.insert(header::USER_AGENT, user_agent),
		Err(e) => {
			println!("Failed to add user agent to header: {e:?}");
			return;
		}
	};
	match HeaderValue::from_str("application/x-www-form-urlencoded") {
		Ok(content_type) => headers.insert(header::CONTENT_TYPE, content_type),
		Err(e) => {
			println!("Failed to add content type to header: {e:?}");
			return;
		}
	};
	let params = [("mode", "delete"), ("index", "0"), ("x", "3"), ("y", "9")];

	match client.post("https://order.subzero.com/instance1/servlet/WebDispatcher?mode=delete&index=0&x=3&y=9").headers(headers).form(&params).send().await {
		Ok(_) => (),
		Err(e) => panic!("Failed to remove item from cart: {e:?}"),
	};
}

///
/// # Add Item
/// Adds an item to the SubZero cart and returns the availablility date.
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
	match my_scroll_table {
		Some(my_scroll_table) => {
			let table_body = my_scroll_table.select(&table_body_selector).next();
			match table_body {
				Some(table_body) => {
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
				}
				None => "Error finding item.".to_string(),
			}
		}
		None => "Error finding item.".to_string(),
	}
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

	println!("SubZero Cookies: {cookies}");
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

	let response_url = response.url().to_string();

	let response_data = match response.text().await {
		Ok(response_data) => response_data,
		Err(e) => return format!("Failed to get suggested items: {e:?}"),
	};

	let response_data = response_data.split('{').collect::<Vec<&str>>()[0].to_string();

	println!("Suggested items: {response_data}");
	println!("Response url: {response_url}");

	response_data
}

///
/// # Login to SubZero System
///
/// ## Outputs
/// bool - True if login was successful, false otherwise.
///
pub async fn subzero_login(username: String, password: String) {
	let mut headers = HeaderMap::new();
	headers.insert(header::USER_AGENT, " Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/99.0.4844.51 Safari/537.36 Edg/99.0.1150.30".parse().unwrap());
	headers.insert(header::ACCESS_CONTROL_ALLOW_CREDENTIALS, "true".parse().unwrap());

	let response = reqwest::Client::new().post("https://order.subzero.com/instance1/servlet/WebDispatcher").headers(headers).form(&[("user", username.as_str()), ("psswd", password.as_str()), ("mode", "logon"), ("env", "EnvZZ")]).send().await.unwrap();

	// get response cookies into json
	let mut cookies_json_vec: Vec<serde_json::Value> = Vec::new();
	let mut subzero_cookies: Vec<PlaywrightCookie> = Vec::new();

	for cookie in response.cookies() {
		let name = cookie.name().to_string();
		let value = cookie.value().to_string();
		let expires = match cookie.expires() {
			Some(expires) => {
				let time: DateTime<Utc> = expires.into();
				time.to_string()
			}
			None => "".to_string(),
		};
		let max_age = match cookie.max_age() {
			Some(max_age) => DurationString::from(max_age).into(),
			None => "".to_string(),
		};
		let domain = match cookie.domain() {
			Some(domain) => domain.to_string(),
			None => "".to_string(),
		};
		let path = match cookie.path() {
			Some(path) => path.to_string(),
			None => "".to_string(),
		};

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
		let expires = match cookie["expires"].as_str().unwrap().parse::<f64>() {
			Ok(expires) => Some(expires),
			Err(_) => None,
		};
		let domain = cookie["domain"].as_str().map(|domain| domain.to_string());
		let path = cookie["path"].as_str().map(|path| path.to_string());
		let name = cookie["name"].as_str().unwrap().to_owned();
		let value = cookie["value"].as_str().unwrap().to_owned();
		let new_cookie: PlaywrightCookie = PlaywrightCookie { name, value, expires, domain, path, url: None, secure: None, http_only: None, same_site: None };
		subzero_cookies.push(new_cookie);
	}

	if !subzero_cookies.is_empty() {
		let token_json = json!({ "token": SubZeroJWTTokenClaims::encode(subzero_cookies).await.unwrap() }).to_string();
		let mut file = File::create("/easfiles/appliances/cookies/subzero_cookies.json").unwrap();
		file.write_all(token_json.as_bytes()).unwrap();
	}
}
