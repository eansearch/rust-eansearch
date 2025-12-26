#![allow(dead_code)]

//! # EANSearch
//!
//! A library to search the EAN barcode database at [EAN-Search.org](https://www.ean-search.org)
//!
//! (c) 2025 Relaxed Communications GmbH <info@relaxedcommunications.com>
//!
//! See [https://www.ean-search.org/ean-database-api.html](https://www.ean-search.org/ean-database-api.html)

use std::{fmt, thread, time};
use std::error::Error;
use serde::Deserialize;
use serde_with::{DisplayFromStr, serde_as};
use serde_json::Value;
use base64::{Engine as _, engine::general_purpose};

/// A product returned from the EAN database
#[serde_as]
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Product {
    #[serde_as(as = "DisplayFromStr")]
    pub ean: u64,
    pub name: String,
    #[serde_as(as = "DisplayFromStr")]
    pub category_id: i32,
    pub category_name: String,
    pub issuing_country: String,
}

impl std::fmt::Display for Product {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "EAN {}: {} (category {}: {}) from {}", self.ean, self.name, self.category_id, self.category_name, self.issuing_country)
    }
}

/// A product returned from the EAN database (extended version)
#[serde_as]
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExtProduct {
    #[serde_as(as = "DisplayFromStr")]
    pub ean: u64,
    pub name: String,
    #[serde_as(as = "DisplayFromStr")]
    pub category_id: i32,
    pub category_name: String,
    #[serde_as(as = "DisplayFromStr")]
    pub google_category_id: i32,
    pub issuing_country: String,
}

impl std::fmt::Display for ExtProduct {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "EAN {}: {} (category {}: {}, google category {}) from {}", self.ean, self.name, self.category_id, self.category_name, self.google_category_id, self.issuing_country)
    }
}

#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProductCountry {
    #[serde_as(as = "DisplayFromStr")]
    ean: u64,
    issuing_country: String,
}

#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct VerifyChecksum {
    #[serde_as(as = "DisplayFromStr")]
    ean: u64,
    valid: String,
}

#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct BarcodeImage {
    #[serde_as(as = "DisplayFromStr")]
    ean: u64,
    barcode: String,
}

#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AccountStatus {
    id: String,
    requests: u32,
    requestlimit: u32,
}

#[serde_as]
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct APIError {
    error: String,
}

const MAX_API_TRIES: i32 = 3;

/// The access object to make API requests to the EAN database
pub struct EANSearch {
	client: reqwest::blocking::Client,
    base_url: String,
	remaining: i64,
}

impl EANSearch {
    /// Construct the database access object with your API token
    pub fn new(token: &str) -> Self {
		let client = reqwest::blocking::Client::builder().user_agent("rust-eansearch/1.0").build().unwrap();
        let base_url = String::from("https://api.ean-search.org/api?format=json&token=") + &token;
		let remaining = -1;
        Self { client, base_url, remaining }
    }

    /// Search for a product by EAN barcode
    pub fn barcode_lookup(&mut self, ean: u64, language: Option<i8>) -> Result<Option<ExtProduct>, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=barcode-lookup&ean=" + &ean.to_string()
            + "&language=" + &language.unwrap_or(1).to_string();
        let body = self.api_call(&url).unwrap();
        let json : Result<Option<Vec<ExtProduct>>, serde_json::Error> = serde_json::from_str(&body);
        match json {
            Ok(p) => Ok(Some(p.unwrap()[0].clone())), // EAN found
            Err(_e) =>  {
                let api_error : Result<Vec<APIError>, serde_json::Error> = serde_json::from_str(&body);
                match api_error {
                    Ok(e) => {
                        if e[0].error == "Barcode not found" {
                            Ok(None)    // Rust has a better way to represent EAN not found
                        } else {
                            Err(e[0].error.clone().into()) // API error
                        }
                    }
                    Err(_e) => Err("Undefined API error".into())
                }
            },
        }
    }

    /// Lookup a book by ISBN-10 or ISBN-13 code
    pub fn isbn_lookup(&mut self, isbn: u64) -> Result<Option<ExtProduct>, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=barcode-lookup&isbn=" + &isbn.to_string();
        let body = self.api_call(&url).unwrap();
        let json : Result<Option<Vec<ExtProduct>>, serde_json::Error> = serde_json::from_str(&body);
        match json {
            Ok(p) => Ok(Some(p.unwrap()[0].clone())), // EAN found
            Err(_e) =>  {
                let api_error : Result<Vec<APIError>, serde_json::Error> = serde_json::from_str(&body);
                match api_error {
                    Ok(e) => {
                        if e[0].error == "Barcode not found" {
                            Ok(None)    // Rust has a better way to represent EAN not found
                        } else {
                            Err(e[0].error.clone().into()) // API error
                        }
                    }
                    Err(_e) => Err("Undefined API error".into())
                }
            },
        }
    }

    fn api_call(&mut self, url: &String) -> Result<String, Box<dyn Error>> {
        let resp = self.client.get(url).send().unwrap();
		if let Some(credits) = resp.headers().get("x-credits-remaining") {
			self.remaining = credits.to_str().unwrap().parse().unwrap();
		} else {
			self.remaining = -1;
		}
        return Ok(resp.text()?);
	}

    fn api_call_list(&mut self, url: &String, tries: i32) -> Result<Vec<Product>, Box<dyn Error>> {
        let resp = self.client.get(url).send().unwrap();
		if let Some(credits) = resp.headers().get("x-credits-remaining") {
			self.remaining = credits.to_str().unwrap().parse().unwrap();
		} else {
			self.remaining = -1;
		}
		if resp.status() == 429 && tries <= MAX_API_TRIES {
			thread::sleep(time::Duration::new(0, 1)); // wait 1 sec
			return self.api_call_list(&url, tries + 1)
		}
        let body = resp.text()?;
        let api_error : Result<Vec<APIError>, serde_json::Error> = serde_json::from_str(&body);
        if api_error.is_ok() {
            return Err(api_error.unwrap()[0].error.clone().into()); // API error
        }
        let json : Value = serde_json::from_str(&body)?;
        let pl = &json["productlist"];
        let json_list = serde_json::to_string(pl);
        let result : Vec<Product> = serde_json::from_str(&json_list.unwrap())?;
        Ok(result)
    }

    /// Search for all products with an EAN barcode staring with this prefix
    pub fn barcode_prefix_search(&mut self, prefix: u64, language: Option<i8>, page: Option<i32>) -> Result<Vec<Product>, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=barcode-prefix-search&prefix=" + &prefix.to_string()
            + "&page=" + &page.unwrap_or(0).to_string()
            + "&language=" + &language.unwrap_or(1).to_string();
		self.api_call_list(&url, 1)
    }

    /// Search for all products matching all keywords in name parameter
    pub fn product_search(&mut self, name: &str, language: Option<i8>, page: Option<i32>) -> Result<Vec<Product>, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=product-search&name=" + name
            + "&language=" + &language.unwrap_or(99).to_string()
            + "&page=" + &page.unwrap_or(0).to_string();
		self.api_call_list(&url, 1)
    }

    /// Search for products with similar keywords
    pub fn similar_product_search(&mut self, name: &str, language: Option<i8>, page: Option<i32>) -> Result<Vec<Product>, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=similar-product-search&name=" + name
            + "&language=" + &language.unwrap_or(99).to_string()
            + "&page=" + &page.unwrap_or(0).to_string();
		self.api_call_list(&url, 1)
    }

    /// Search for all products in a product catgory, optionally restricted by keywords in the name parameter
    pub fn category_search(&mut self, category: i32, name: Option<&str>, language: Option<i8>, page: Option<i32>) -> Result<Vec<Product>, Box<dyn Error>> {
        let mut url : String = self.base_url.to_owned()
            + "&op=category-search&category=" + &category.to_string();
        if name.is_some() {
            url = url + "&name=" + name.unwrap();
        };
        url = url + "&language=" + &language.unwrap_or(99).to_string()
            + "&page=" + &page.unwrap_or(0).to_string();
		self.api_call_list(&url, 1)
    }

    /// Query the country that issued an EAN barcode (available, even if we don't have specific in formation on the product)
    pub fn issuing_country(&mut self, ean: u64) -> Result<String, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=issuing-country&ean=" + &ean.to_string();
        let body = self.api_call(&url).unwrap();
        let json : Result<Vec<ProductCountry>, serde_json::Error> = serde_json::from_str(&body);
        match json {
            Ok(p) => Ok(p[0].issuing_country.clone()),
            Err(_e) =>  {
                let api_error : Result<Vec<APIError>, serde_json::Error> = serde_json::from_str(&body);
                match api_error {
                    Ok(e) => Err(e[0].error.clone().into()),
                    Err(_e) => Err("Undefined API error".into()),
                }
            },
        }
    }

    /// Verify if the provided number is a valid EAN barcode
    pub fn verify_checksum(&mut self, ean: u64) -> Result<bool, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=verify-checksum&ean=" + &ean.to_string();
        let body = self.api_call(&url).unwrap();
        let json : Result<Vec<VerifyChecksum>, serde_json::Error> = serde_json::from_str(&body);
        match json {
            Ok(p) => Ok(p[0].valid == "1"),
            Err(_e) =>  {
                let api_error : Result<Vec<APIError>, serde_json::Error> = serde_json::from_str(&body);
                match api_error {
                    Ok(e) => Err(e[0].error.clone().into()),
                    Err(_e) => Err("Undefined API error".into()),
                }
            },
        }
    }

    /// Get a PNG image of the EAN barcode
    pub fn barcode_image(&mut self, ean: u64, width: Option<i32>, height: Option<i32>) -> Result<Vec<u8>, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=barcode-image&ean=" + &ean.to_string()
            + "&width=" + &width.unwrap_or(102).to_string() + "&height=" + &height.unwrap_or(50).to_string();
        let body = self.api_call(&url).unwrap();
        let json : Result<Vec<BarcodeImage>, serde_json::Error> = serde_json::from_str(&body);
        match json {
            Ok(p) => Ok(general_purpose::STANDARD_NO_PAD.decode(&p[0].barcode).unwrap()),
            Err(_e) =>  {
                let api_error : Result<Vec<APIError>, serde_json::Error> = serde_json::from_str(&body);
                match api_error {
                    Ok(e) => Err(e[0].error.clone().into()),
                    Err(_e) => Err("Undefined API error".into()),
                }
            },
        }
    }

    /// Check how many requests are still available for your account in this payment cycle
    pub fn credits_remaining(&mut self) -> i64 {
		if self.remaining < 0 {
			let url : String = self.base_url.to_owned() + "&op=account-status";
			let _ = self.api_call(&url).unwrap();
		}
		self.remaining
	}

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_barcode_lookup() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
        let product = eansearch.barcode_lookup(5099750442227, Some(1));
        assert!(product.is_ok()); // check if API call went through ok
        let product = product.unwrap(); // extract from Result
        assert!(product.is_some()); // check if a product was found
        let product = product.unwrap();
        assert!(product.name.contains("Thriller"));
        assert_eq!(product.category_id, 45);
        assert_eq!(product.category_name, "Music");
        assert_eq!(product.google_category_id, 855);
        assert_eq!(product.issuing_country, "UK");
    }

    #[test]
    fn test_barcode_lookup_invalid() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
        let product = eansearch.barcode_lookup(1, None);
        assert!(product.is_err());
    }

    #[test]
    fn test_barcode_lookup_not_found() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
		//let credits_before = eansearch.credits_remaining();
        let product = eansearch.barcode_lookup(4603300350552, None);
        if product.is_err() {
            println!("Error = {:?}", product.as_ref().err())
        }
		//let credits_after = eansearch.credits_remaining();
        assert!(product.is_ok());
        assert!(!product.unwrap().is_some());
        //assert!(credits_before > credits_after);
    }

    #[test]
    fn test_barcode_lookup_api_error() {
        let mut eansearch = EANSearch::new("xxx"); // invalid token
        let product = eansearch.barcode_lookup(5099750442227, None);
        if product.is_err() {
            println!("Error = {:?}", product.as_ref().err())
        }
        assert!(product.is_err());
        let msg = format!("{:?}", product.as_ref().err());
        assert!(msg == "Some(\"Invalid token\")");
    }

    #[test]
    fn test_isbn_lookup() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
        let product = eansearch.isbn_lookup(1119578884);
        assert!(product.is_ok()); // check if API call went through ok
        let product = product.unwrap(); // extract from Result
        assert!(product.is_some()); // check if a product was found
        let product = product.unwrap();
        assert!(product.name.contains("Linux"));
        assert_eq!(product.category_id, 15);
        assert_eq!(product.category_name, "Books and Magazines");
        assert_eq!(product.google_category_id, 784);
    }

    #[test]
    fn test_barcode_prefix_search() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
		//let credits_before = eansearch.credits_remaining();
        let product_list = eansearch.barcode_prefix_search(509975044, Some(1), None);
        assert!(product_list.is_ok());
        assert!(!product_list.as_ref().unwrap().is_empty());
        for p in &product_list.unwrap() {
            println!("Result: {:0>13} = {} ({})", p.ean, p.name, p.category_id);
        }
		//let credits_after = eansearch.credits_remaining();
        //assert!(credits_before > credits_after);
    }

    #[test]
    fn test_barcode_prefix_search_too_short() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
        let product_list = eansearch.barcode_prefix_search(5, Some(1), None);
        if product_list.is_err() {
            println!("Error = {:?}", product_list.as_ref().err())
        }
        assert!(product_list.is_err());
    }

    #[test]
    fn test_product_search() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
        let product_list = eansearch.product_search("bananaboat", Some(1), None);
        assert!(product_list.is_ok());
        assert!(!product_list.as_ref().unwrap().is_empty());
        for p in &product_list.unwrap() {
            println!("Result: {:0>13} = {} ({})", p.ean, p.name, p.category_id);
        }
    }

    #[test]
    fn test_product_search_not_found() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
        let product_list = eansearch.product_search("WordNever2BFound", Some(1), None);
        assert!(product_list.is_ok());
        assert!(product_list.as_ref().unwrap().is_empty());
    }

    #[test]
    fn test_similar_product_search_found() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
        let product_list = eansearch.similar_product_search("bananaboat WordNever2BFound", Some(1), None);
        assert!(product_list.is_ok());
        assert!(!product_list.as_ref().unwrap().is_empty());
        for p in &product_list.unwrap() {
            println!("Result: {:0>13} = {} ({})", p.ean, p.name, p.category_id);
        }
    }

    #[test]
    fn test_product_search_api_error() {
        let mut eansearch = EANSearch::new("xxx"); // invalid token
        let product_list = eansearch.product_search("bananaboat", Some(1), None);
        if product_list.is_err() {
            println!("Error = {:?}", product_list.as_ref().err())
        }
        assert!(product_list.is_err());
        let msg = format!("{:?}", product_list.as_ref().err());
        assert!(msg == "Some(\"Invalid token\")");
    }

    #[test]
    fn test_category_search() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
        let product_list = eansearch.category_search(45, Some("bananaboat"), Some(1), None);
        assert!(product_list.is_ok());
        assert!(!product_list.as_ref().unwrap().is_empty());
        for p in &product_list.unwrap() {
            println!("Result: {:0>13} = {} ({})", p.ean, p.name, p.category_id);
        }
    }

    #[test]
    fn test_issuing_country() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
        let country_lookup = eansearch.issuing_country(5099750442227);
        if country_lookup.is_err() {
            println!("Error = {:?}", country_lookup.as_ref().err())
        }
        assert!(country_lookup.is_ok());
        let country = country_lookup.unwrap();
        assert!(country == "UK");
    }

    #[test]
    fn test_verify_checksum() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
        let checksum_ok = eansearch.verify_checksum(5099750442227);
        assert!(checksum_ok.is_ok());
        assert!(checksum_ok.unwrap() == true);
    }

    #[test]
    fn test_verify_checksum_fail() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
        let checksum_ok = eansearch.verify_checksum(1);
        assert!(checksum_ok.is_ok());
        assert!(checksum_ok.unwrap() == false);
    }

    #[test]
    fn test_barcode_image() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let mut eansearch = EANSearch::new(&token);
        let img = eansearch.barcode_image(5099750442227, None, None);
        assert!(img.is_ok());
    }

}
