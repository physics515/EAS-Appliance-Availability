#![allow(dead_code)]
use std::sync::Arc;

use azure_identity::ImdsManagedIdentityCredential;
use azure_security_keyvault::KeyvaultClient;
pub use bsh::{bsh_availability, bsh_login};
use chrono::Utc;
use eggersmann_app_server_auth::User;
pub use miele::miele_availability;
use serde::{Deserialize, Serialize};
pub use subzero::{subzero_availability, subzero_login};

mod bsh;
mod miele;
mod subzero;

///
/// # AvailabilityRequestUser
/// User struct for use in the availability request.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailabilityRequestUser {
	pub id: String,
	pub given_name: Option<String>,
	pub surname: Option<String>,
	pub display_name: Option<String>,
	pub job_title: Option<String>,
	pub user_principal_name: Option<String>,
	pub office_location: Option<String>,
}

///
/// # AvailabilityRequest
/// Struct for use in the availability request.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailabilityRequest {
	pub manufacturer: Option<String>,
	pub showroom: Option<String>,
	pub model_number: Option<String>,
	pub warehouse: Option<String>,
	pub utc_time: Option<String>,
	pub availability: Option<String>,
	pub user: Option<AvailabilityRequestUser>,
}

impl AvailabilityRequest {
	///
	/// # AvailabilityRequest::new
	/// Create new AvailabilityRequest.
	///
	/// ## Inputs
	/// * `manufacturer`: String - The manufacturer of the appliance.
	/// * `showroom`: String - The showroom location of the user.
	/// * `model_number`: String - The model of the appliance.
	///
	/// ## Outputs
	/// AvailabilityRequest - The new AvailabilityRequest.
	///
	/// ## Example
	/// ```
	/// use crate::router::api_v1::availability::AvailabilityRequest;
	/// let req: AvailabilityRequest = AvailabilityRequest::new("bsh", "houston", "HBLP651RUC");
	/// ```
	pub fn new(manufacturer: String, showroom: String, model_number: String) -> Self {
		AvailabilityRequest { manufacturer: Some(manufacturer), showroom: Some(showroom), model_number: Some(model_number), warehouse: None, utc_time: None, availability: None, user: None }
	}

	///
	/// # AvailabilityRequest::add_user
	/// Add user to AvailabilityRequest.
	///
	/// ## Inputs
	/// * `user`: AvailabilityRequestUser - The user to add to the AvailabilityRequest.
	///
	/// ## Outputs
	/// AvailabilityRequest - The AvailabilityRequest with the user added.
	///
	/// ## Example
	/// ```
	/// use crate::router::api_v1::availability::AvailabilityRequest;
	///
	/// #[get("/user/title")]
	/// pub async fn get_user_title(user: User) -> String {
	///     let req: AvailabilityRequest = AvailabilityRequest::new("bsh", "houston", "HBLP651RUC");
	///     req.add_user(user);
	///     req.user.job_title.to_string()
	/// }
	/// ```
	///
	pub async fn add_user(mut self, user: User) -> Self {
		self.user = Some(AvailabilityRequestUser {
			id: user.token.id,
			given_name: user.token.given_name,
			surname: user.token.surname,
			display_name: user.token.display_name,
			job_title: user.token.job_title,
			user_principal_name: user.token.user_principal_name,
			office_location: user.token.office_location,
		});
		self
	}

	///
	/// # AvailabilityRequest::parse_manufacturer
	/// Parse the manufacturer from the request.
	///
	/// ## Example
	/// ```
	/// use crate::router::api_v1::appliances::availability::AvailabilityRequest;
	///
	/// let req: AvailabilityRequest = AvailabilityRequest::new("BSH", "houston", "HBLP651RUC");
	/// req.parse_manufacturer();
	/// assert_eq!(req.manufacturer, "bsh");
	/// ```
	///
	pub async fn parse_manufacturer(mut self) -> Self {
		match self.manufacturer {
			Some(manufacturer) => match manufacturer.to_lowercase().as_str() {
				"bsh" => {
					self.manufacturer = Some("bsh".to_string());
					self
				}
				"subzero" => {
					self.manufacturer = Some("subzero".to_string());
					self
				}
				"miele" => {
					self.manufacturer = Some("miele".to_string());
					self
				}
				_ => {
					self.manufacturer = None;
					self
				}
			},
			None => {
				self.manufacturer = None;
				self
			}
		}
	}

	///
	/// # AvailabilityRequest::get_warehouse
	/// Get the warehouse from the request and pasrse it into a format that can be read by the manufacture interface.
	///
	pub async fn get_warehouse(mut self) -> Self {
		match self.showroom.clone() {
			Some(showroom) => match showroom.to_lowercase().as_str() {
				"houston" => match self.manufacturer.clone() {
					Some(manufacturer) => match manufacturer.to_lowercase().as_str() {
						"bsh" => {
							self.warehouse = Some("US00002148".to_string());
							self
						}
						"subzero" => {
							self.warehouse = Some("99432040".to_string());
							self
						}
						"miele" => {
							self.warehouse = Some("Forest Park, IL".to_string());
							self
						}
						_ => {
							self.warehouse = None;
							self
						}
					},
					_ => {
						self.warehouse = None;
						self
					}
				},
				"florida" => match self.manufacturer.clone() {
					Some(manufacturer) => match manufacturer.to_lowercase().as_str() {
						"bsh" => {
							self.warehouse = Some("US00000103".to_string());
							self
						}
						"subzero" => {
							self.warehouse = Some("99211620".to_string());
							self
						}
						"miele" => {
							self.warehouse = Some("Pompano Beach, FL".to_string());
							self
						}
						_ => {
							self.warehouse = None;
							self
						}
					},
					_ => {
						self.warehouse = None;
						self
					}
				},
				"los angeles" => match self.manufacturer.clone() {
					Some(manufacturer) => match manufacturer.to_lowercase().as_str() {
						"bsh" => {
							self.warehouse = Some("US00003803".to_string());
							self
						}
						"subzero" => {
							self.warehouse = Some("99614560".to_string());
							self
						}
						"miele" => {
							self.warehouse = Some("Stockton, CA".to_string());
							self
						}
						_ => {
							self.warehouse = None;
							self
						}
					},
					_ => {
						self.warehouse = None;
						self
					}
				},
				"chicago" => match self.manufacturer.clone() {
					Some(manufacturer) => match manufacturer.to_lowercase().as_str() {
						"bsh" => {
							self.warehouse = Some("US00001842".to_string());
							self
						}
						"subzero" => {
							self.warehouse = Some("99311630".to_string());
							self
						}
						"miele" => {
							self.warehouse = Some("Forest Park, IL".to_string());
							self
						}
						_ => {
							self.warehouse = None;
							self
						}
					},
					_ => {
						self.warehouse = None;
						self
					}
				},
				"new york" => match self.manufacturer.clone() {
					Some(manufacturer) => match manufacturer.to_lowercase().as_str() {
						"bsh" => {
							self.warehouse = Some("US00002933".to_string());
							self
						}
						"subzero" => {
							self.warehouse = Some("99103710".to_string());
							self
						}
						"miele" => {
							self.warehouse = Some("South Brunswick, NJ".to_string());
							self
						}
						_ => {
							self.warehouse = None;
							self
						}
					},
					_ => {
						self.warehouse = None;
						self
					}
				},
				"dallas" => match self.manufacturer.clone() {
					Some(manufacturer) => match manufacturer.to_lowercase().as_str() {
						"bsh" => {
							self.warehouse = Some("US00003189".to_string());
							self
						}
						"subzero" => {
							self.warehouse = Some("99411540".to_string());
							self
						}
						"miele" => {
							self.warehouse = Some("Forest Park, IL".to_string());
							self
						}
						_ => {
							self.warehouse = None;
							self
						}
					},
					_ => {
						self.warehouse = None;
						self
					}
				},
				_ => {
					self.warehouse = None;
					self
				}
			},
			None => {
				self.warehouse = None;
				self
			}
		}
	}

	///
	/// # AvailabilityRequest::get_time
	/// Get the current time in the format %m/%d/%Y %I:%M:%S %p
	///
	pub async fn get_time(mut self) -> Self {
		let utc_time = Utc::now().format("%m/%d/%Y %I:%M:%S %p").to_string();
		self.utc_time = Some(utc_time);
		self
	}

	///
	/// # AvailabilityRequest::get_availability
	/// Get the availability for the requested product.
	///
	pub async fn get_availability(mut self) -> Self {
		match self.manufacturer.clone() {
			Some(manufacturer) => match manufacturer.to_lowercase().as_str() {
				"bsh" => {
					let azure_credentials = ImdsManagedIdentityCredential::default();
					let client = KeyvaultClient::new("https://eggappserverkeyvault.vault.azure.net", Arc::new(azure_credentials)).unwrap();
					let bsh_username = client.secret_client().get("bsh-username").await.unwrap().value;
					let bsh_password = client.secret_client().get("bsh-password").await.unwrap().value;
					self.availability = Some(bsh::bsh_availability(self.clone(), bsh_username, bsh_password).await);
					self
				}
				"subzero" => {
					let azure_credentials = ImdsManagedIdentityCredential::default();
					let client = KeyvaultClient::new("https://eggappserverkeyvault.vault.azure.net", Arc::new(azure_credentials)).unwrap();
					let subzero_username = client.secret_client().get("subzero-username").await.unwrap().value;
					let subzero_password = client.secret_client().get("subzero-password").await.unwrap().value;
					self.availability = Some(subzero::subzero_availability(self.clone(), subzero_username, subzero_password).await);
					self
				}
				"miele" => {
					self.availability = Some(miele::miele_availability(self.clone()).await);
					self
				}
				_ => {
					self.availability = None;
					self
				}
			},
			None => {
				self.availability = None;
				self
			}
		}
	}
}
