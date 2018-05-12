extern crate gdpr_consent;

use gdpr_consent::vendor::{from_str, to_str, VendorConsent};

fn main() {
    let serialized = "BOEFBi5OEFBi5AHABDENAI4AAAB9vABAASA";
    let vendor_consent = from_str(serialized).unwrap();
    let VendorConsent::V1(mut v1) = vendor_consent;

    v1.last_updated = 1526040000000; // 2018-05-11T12:00:00.000Z
    v1.vendor_consent.remove(9); // remove consent for Vendor ID 10

    let serialized = to_str(VendorConsent::V1(v1)).unwrap();
    assert_eq!(serialized, "BOEFBi5ONlzmAAHABDENAI4AAAB9vABgASABQA");
}
