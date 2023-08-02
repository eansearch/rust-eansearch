#![allow(dead_code)]

//! # EANSearch
//!
//! A library to search the EAN barcode database at [EAN-Search.org](https://www.ean-search.org)
//!
//! (c) 2023 Relaxed Communications GmbH <info@relaxedcommunications.com>
//!
//! See [https://www.ean-search.org/ean-database-api.html](https://www.ean-search.org/ean-database-api.html)

use std::fmt;
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
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct APIError {
    error: String,
}

/// The access object to make API requests to the EAN database
pub struct EANSearch {
    base_url: String,
}

impl EANSearch {
    /// Construct the database access object with your API token
    pub fn new(token: &str) -> Self {
        let url = String::from("https://api.ean-search.org/api?format=json&token=") + &token;
        Self { base_url: url }
    }

    /// Search for a product by EAN barcode
    pub fn barcode_lookup(&self, ean: u64, language: Option<i8>) -> Result<Option<Product>, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=barcode-lookup&ean=" + &ean.to_string()
            + "&language=" + &language.unwrap_or(1).to_string();
        let body = reqwest::blocking::get(url)?.text()?;
        let json : Result<Option<Vec<Product>>, serde_json::Error> = serde_json::from_str(&body);
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

    /// Search for all products with an EAN barcode staring with this prefix
    pub fn barcode_prefix_search(&self, prefix: u64, language: Option<i8>, page: Option<i32>) -> Result<Vec<Product>, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=barcode-prefix-search&prefix=" + &prefix.to_string()
            + "&page=" + &page.unwrap_or(0).to_string()
            + "&language=" + &language.unwrap_or(1).to_string();
        let body = reqwest::blocking::get(url)?.text()?;
        let json : Value = serde_json::from_str(&body)?;
        let pl = &json["productlist"];
        let json_list = serde_json::to_string(pl);
        let result : Vec<Product> = serde_json::from_str(&json_list.unwrap())?;
        // TODO: catch API error
        // TODO: signal total list size?
        Ok(result)
    }

    /// Search for all products matching all keywords in name parameter
    pub fn product_search(&self, name: &str, language: Option<i8>, page: Option<i32>) -> Result<Vec<Product>, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=product-search&name=" + name
            + "&language=" + &language.unwrap_or(99).to_string()
            + "&page=" + &page.unwrap_or(0).to_string();
        let body = reqwest::blocking::get(url)?.text()?;
        let json : Value = serde_json::from_str(&body)?;
        let pl = &json["productlist"];
        let json_list = serde_json::to_string(pl);
        let result : Vec<Product> = serde_json::from_str(&json_list.unwrap())?;
        // TODO: catch API error
        // TODO: signal total list size?
        Ok(result)
    }

    /// Search for all products in a product catgory, optionally restricted by keywords in the name parameter
    pub fn category_search(&self, category: i32, name: Option<&str>, language: Option<i8>, page: Option<i32>) -> Result<Vec<Product>, Box<dyn Error>> {
        let mut url : String = self.base_url.to_owned()
            + "&op=category-search&category=" + &category.to_string();
        if name.is_some() {
            url = url + "&name=" + name.unwrap();
        };
        url = url + "&language=" + &language.unwrap_or(99).to_string()
            + "&page=" + &page.unwrap_or(0).to_string();
        let body = reqwest::blocking::get(url)?.text()?;
        let json : Value = serde_json::from_str(&body)?;
        let pl = &json["productlist"];
        let json_list = serde_json::to_string(pl);
        let result : Vec<Product> = serde_json::from_str(&json_list.unwrap())?;
        // TODO: catch API error
        // TODO: signal total list size?
        Ok(result)
    }

    /// Query the country that issued an EAN barcode (available, even if we don't have specific in formation on the product)
    pub fn issuing_country(&self, ean: u64) -> Result<String, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=issuing-country&ean=" + &ean.to_string();
        let body = reqwest::blocking::get(url)?.text()?;
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
    pub fn verify_checksum(&self, ean: u64) -> Result<bool, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=verify-checksum&ean=" + &ean.to_string();
        let body = reqwest::blocking::get(url)?.text()?;
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

    /// Check how many requests are still available for your account in this payment cycle
    pub fn account_status(&self) -> Result<u32, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=account-status";
        let body = reqwest::blocking::get(url)?.text()?;
        let json : Result<AccountStatus, serde_json::Error> = serde_json::from_str(&body);
        match json {
            Ok(s) => Ok(s.requestlimit - s.requests),
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
    pub fn barcode_image(&self, ean: u64, width: Option<i32>, height: Option<i32>) -> Result<Vec<u8>, Box<dyn Error>> {
        let url : String = self.base_url.to_owned()
            + "&op=barcode-image&ean=" + &ean.to_string()
            + "&width=" + &width.unwrap_or(102).to_string() + "&height=" + &height.unwrap_or(50).to_string();
        let body = reqwest::blocking::get(url)?.text()?;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_barcode_lookup() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let eansearch = EANSearch::new(&token);
        let product = eansearch.barcode_lookup(5099750442227, Some(1));
        assert!(product.is_ok()); // check if API call went through ok
        let product = product.unwrap(); // extract from Result
        assert!(product.is_some()); // check if a product was found
        let product = product.unwrap();
        assert!(product.name.contains("Thriller"));
        assert_eq!(product.category_id, 45);
        assert_eq!(product.category_name, "Music");
        assert_eq!(product.issuing_country, "UK");
    }

    #[test]
    fn test_barcode_lookup_invalid() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let eansearch = EANSearch::new(&token);
        let product = eansearch.barcode_lookup(1, None);
        assert!(product.is_err());
    }

    #[test]
    fn test_barcode_lookup_not_found() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let eansearch = EANSearch::new(&token);
        let product = eansearch.barcode_lookup(4603300350552, None);
        if product.is_err() {
            println!("Error = {:?}", product.as_ref().err())
        }
        assert!(product.is_ok());
        assert!(!product.unwrap().is_some());
    }

    #[test]
    fn test_barcode_lookup_api_error() {
        let eansearch = EANSearch::new("xxx"); // invalid token
        let product = eansearch.barcode_lookup(5099750442227, None);
        if product.is_err() {
            println!("Error = {:?}", product.as_ref().err())
        }
        assert!(product.is_err());
        let msg = format!("{:?}", product.as_ref().err());
        assert!(msg == "Some(\"Invalid token\")");
    }

    #[test]
    fn test_barcode_prefix_search() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let eansearch = EANSearch::new(&token);
        let product_list = eansearch.barcode_prefix_search(509975044, Some(1), None);
        assert!(product_list.is_ok());
        assert!(!product_list.as_ref().unwrap().is_empty());
        for p in &product_list.unwrap() {
            println!("Result: {:0>13} = {} ({})", p.ean, p.name, p.category_id);
        }
    }

    #[test]
    fn test_barcode_prefix_search_too_short() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let eansearch = EANSearch::new(&token);
        let product_list = eansearch.barcode_prefix_search(50, Some(1), None);
        if product_list.is_err() {
            println!("Error = {:?}", product_list.as_ref().err())
        }
        assert!(product_list.is_err());
    }

    #[test]
    fn test_product_search() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let eansearch = EANSearch::new(&token);
        let search_term = "bananaboat";
        let product_list = eansearch.product_search(search_term, Some(1), None);
        assert!(product_list.is_ok());
        assert!(!product_list.as_ref().unwrap().is_empty());
        for p in &product_list.unwrap() {
            println!("Result: {:0>13} = {} ({})", p.ean, p.name, p.category_id);
        }
    }

    #[test]
    fn test_product_search_not_found() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let eansearch = EANSearch::new(&token);
        let search_term = "WordNever2BFound"; // stop word, no results
        let product_list = eansearch.product_search(search_term, Some(1), None);
        assert!(product_list.is_ok());
        assert!(product_list.as_ref().unwrap().is_empty());
    }

    #[test]
    fn test_category_search() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let eansearch = EANSearch::new(&token);
        let search_term = "bananaboat";
        let product_list = eansearch.category_search(45, Some(search_term), Some(1), None);
        assert!(product_list.is_ok());
        assert!(!product_list.as_ref().unwrap().is_empty());
        for p in &product_list.unwrap() {
            println!("Result: {:0>13} = {} ({})", p.ean, p.name, p.category_id);
        }
    }

    #[test]
    fn test_issuing_country() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let eansearch = EANSearch::new(&token);
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
        let eansearch = EANSearch::new(&token);
        let checksum_ok = eansearch.verify_checksum(5099750442227);
        assert!(checksum_ok.is_ok());
        assert!(checksum_ok.unwrap() == true);
    }

    #[test]
    fn test_verify_checksum_fail() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let eansearch = EANSearch::new(&token);
        let checksum_ok = eansearch.verify_checksum(1);
        assert!(checksum_ok.is_ok());
        assert!(checksum_ok.unwrap() == false);
    }

    #[test]
    fn test_barcode_image() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let eansearch = EANSearch::new(&token);
        let img = eansearch.barcode_image(5099750442227, None, None);
        assert!(img.is_ok());
    }

    #[test]
    fn test_account_status() {
        let token = env::var("EAN_SEARCH_API_TOKEN").expect("EAN_SEARCH_API_TOKEN not set");
        let eansearch = EANSearch::new(&token);
        let remaining = eansearch.account_status();
        if remaining.is_err() {
            println!("Error = {:?}", remaining.as_ref().err());
        }
        assert!(remaining.is_ok());
        println!("Remaining requests = {}", remaining.unwrap());
    }

}
