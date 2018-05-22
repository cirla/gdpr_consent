// Copyright (c) 2018 The gdpr_consent authors
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate gdpr_consent;
extern crate reqwest;

use gdpr_consent::vendor_list::VendorList;
use std::error::Error;

fn main() -> Result<(), Box<Error>> {
    let json = reqwest::get("https://vendorlist.consensu.org/vendorlist.json")?.text()?;
    let vendor_list: VendorList = json.parse()?;

    match vendor_list.vendors.get(&32) {
        Some(appnexus) => println!("{:?}", appnexus),
        None => println!("AppNexus was not present in the vendor list."),
    }

    Ok(())
}
