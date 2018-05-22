[![Travis Build Status](https://travis-ci.org/cirla/gdpr_consent.svg?branch=master)](https://travis-ci.org/cirla/gdpr_consent)
[![AppVeyor Build status](https://ci.appveyor.com/api/projects/status/0uccoj1xrgyudp4p/branch/master?svg=true)](https://ci.appveyor.com/project/cirla/gdpr-consent/branch/master)
[![Coverage Status](https://coveralls.io/repos/github/cirla/gdpr_consent/badge.svg?branch=master)](https://coveralls.io/github/cirla/gdpr_consent?branch=master)
[![crates.io](https://img.shields.io/crates/v/gdpr_consent.svg)](https://crates.io/crates/gdpr_consent)
[![docs.rs](https://docs.rs/gdpr_consent/badge.svg)](https://docs.rs/gdpr_consent)

# GDPR Transparency and Consent Framework SDK Rust

## Example Usage

### Vendor List

```rust
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
```

### Vendor Consent String

```rust
extern crate chrono;
extern crate gdpr_consent;

use gdpr_consent::vendor_consent::VendorConsent;
use std::error::Error;

fn main() -> Result<(), Box<Error>> {
    let vendor_consent = "BOEFEAyOEFEAyAHABDENAI4AAAB9vABAASA".parse()?;
    let VendorConsent::V1(mut v1) = vendor_consent;

    v1.last_updated = "2018-05-11T12:00:00.000Z".parse()?;
    v1.vendor_consent.remove(9); // remove consent for Vendor ID 10

    let serialized = VendorConsent::V1(v1).to_string()?;
    assert_eq!(serialized, "BOEFEAyONlzmAAHABDENAI4AAAB9vABgASABQA");

    Ok(())
}
```

