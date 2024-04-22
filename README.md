# EANSearch

Search for products by EAN barcode or product name / keywords

## Features

* Search by EAN code
* Lookup by ISBN code (ISBN-10 or ISBN-13)
* Search by name or keyords
* restrict search by product category
* get the issuing country for the barcode
* verify barcode checksum
* get PNG image for the EAN barcode

## How to use
```rust
// search by EAN barcode, product name in English
let eansearch = EANSearch::new(&token);
let product = eansearch.barcode_lookup(5099750442227, Some(1));
let product = product.unwrap(); // unwrap result
let product = product.unwrap();
println!("EAN {} is {}", product.ean, product.name);

// search by ISBN code
let eansearch = EANSearch::new(&token);
let book = eansearch.isbn_lookup(1119578884);
let book = book.unwrap(); // unwrap result
let book = book.unwrap();
println!("ISBN-13 {} is {}", book.ean, book.name);

// now find all products with the keyword 'bananaboat'
let product_list = eansearch.product_search("bananaboat", Some(1), None);
for p in &product_list.unwrap() {
	println!("EAN {:0>13} is {} ({})", p.ean, p.name, p.category_name);
}

// only find 'bananaboat' products from the 'Music' category
let product_list = eansearch.category_search(45, Some("bananaboat"), Some(1), None);

// download a EANs that start with 509975044xxx
let product_list = eansearch.barcode_prefix_search(509975044, Some(1), None);

// find the country where a barcode was issued
let country_lookup = eansearch.issuing_country(5099750442227);

// check if this is really a valid barcode
let checksum_ok = eansearch.verify_checksum(5099750442227);

// get A PNG image of the barcode to display eg. on a website
let img = eansearch.barcode_image(5099750442227, None, None);

```

To use the library, you need an account and obtain an API token.

See [https://www.ean-search.org/ean-database-api.html](https://www.ean-search.org/ean-database-api.html)
