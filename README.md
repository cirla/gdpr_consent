[![Travis Build Status](https://travis-ci.org/cirla/gdpr_consent.svg?branch=master)](https://travis-ci.org/cirla/gdpr_consent)
[![AppVeyor Build status](https://ci.appveyor.com/api/projects/status/0uccoj1xrgyudp4p/branch/master?svg=true)](https://ci.appveyor.com/project/cirla/gdpr-consent/branch/master)
[![Coverage Status](https://coveralls.io/repos/github/cirla/gdpr_consent/badge.svg?branch=master)](https://coveralls.io/github/cirla/gdpr_consent?branch=master)
[![crates.io](https://img.shields.io/crates/v/gdpr_consent.svg)](https://crates.io/crates/gdpr_consent)
[![docs.rs](https://docs.rs/gdpr_consent/badge.svg)](https://docs.rs/gdpr_consent)

# GDPR Transparency and Consent Framework SDK Rust

## Example Usage

```rust
extern crate gdpr_consent;

use gdpr_consent::vendor_consent::{from_str, to_str, VendorConsent};

fn main() {
    let serialized = "BOEFBi5OEFBi5AHABDENAI4AAAB9vABAASA";
    let vendor_consent = from_str(serialized).unwrap();
    let VendorConsent::V1(mut v1) = vendor_consent;

    v1.last_updated = 1526040000000; // 2018-05-11T12:00:00.000Z
    v1.vendor_consent.remove(9); // remove consent for Vendor ID 10

    let serialized = to_str(VendorConsent::V1(v1)).unwrap();
    assert_eq!(serialized, "BOEFBi5ONlzmAAHABDENAI4AAAB9vABgASABQA");
}
```

