// Copyright (c) 2018 The gdpr_consent authors
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::convert::From;
use std::io;

use base64;
use bit_set::BitSet;
use bit_vec::BitVec;
use bitstream_io::{BitWriter, BE};
use nom;

#[derive(Debug, PartialEq)]
pub struct V1 {
    // Epoch ms when consent string was first created
    pub created: u64,

    // Epoch ms when consent string was last updated
    pub last_updated: u64,

    // Consent Manager Provider ID that last updated the consent string
    pub cmp_id: u16,

    // Consent Manager Provider version
    pub cmp_version: u16,

    // Screen number in the CMP where consent was given
    pub consent_screen: u8,

    // Two-letter ISO639-1 language code that CMP asked for consent in
    pub consent_language: String,

    // Version of vendor list used in most recent consent string update.
    pub vendor_list_version: u16,

    // For each purpose listed in the global vendor list, the presence indicates consent.
    pub purposes_allowed: BitSet,

    // Maximum vendor ID represented in the vendor_consent BitSet.
    pub max_vendor_id: usize,

    // For each vendor id listend in the global vendor list, the presence indicates consent.
    // Vendor IDs are offset by 1 (e.g. bit 0 corresponds with vendor ID 1.
    pub vendor_consent: BitSet,
}

pub enum VendorConsent {
    V1(V1),
}

#[derive(Debug)]
pub enum Error {
    Base64DecodeError(base64::DecodeError),
    UnsupportedVersion(u8),
    IoError(io::Error),
    Other(String),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IoError(e)
    }
}

impl From<base64::DecodeError> for Error {
    fn from(e: base64::DecodeError) -> Error {
        Error::Base64DecodeError(e)
    }
}

impl From<nom::ErrorKind> for Error {
    fn from(e: nom::ErrorKind) -> Error {
        Error::Other(format!("{}", e))
    }
}

named!(take_version<u8>, bits!(take_bits!(u8, 6)));

#[derive(Debug, PartialEq)]
enum Entry {
    Single(usize),
    Range(usize, usize),
}

#[derive(Debug, PartialEq)]
enum Encoding {
    Bitfield,
    Range,
}

#[derive(Debug, PartialEq)]
enum EntryType {
    Single,
    Range,
}

named!(
    parse_v1<V1>,
    bits!(do_parse!(
        version: tag_bits!(u8, 6, 1) >> created: map!(take_bits!(u64, 36), |x| x * 100)
            >> last_updated: map!(take_bits!(u64, 36), |x| x * 100)
            >> cmp_id: take_bits!(u16, 12) >> cmp_version: take_bits!(u16, 12)
            >> consent_screen: take_bits!(u8, 6)
            >> consent_language:
                map!(
                    count!(map!(take_bits!(u8, 6), |x| x + ('a' as u8)), 2),
                    |x| String::from_utf8(x).unwrap()
                ) >> vendor_list_version: take_bits!(u16, 12)
            >> purposes_allowed: map!(count!(take_bits!(u8, 8), 3), |x| BitSet::from_bytes(&x))
            >> max_vendor_id: take_bits!(usize, 16)
            >> encoding_type: map!(take_bits!(u8, 1), |x| {
                if x == 0 {
                    Encoding::Bitfield
                } else {
                    Encoding::Range
                }
            })
            >> bitfield_section:
                cond!(
                    encoding_type == Encoding::Bitfield,
                    do_parse!(
                        full_bytes: count!(take_bits!(u8, 8), max_vendor_id / 8)
                            >> leftover_byte:
                                cond!(max_vendor_id % 8 > 0, take_bits!(u8, max_vendor_id % 8))
                            >> (match leftover_byte {
                                Some(b) => {
                                    let mut bitset = BitSet::from_bytes(&full_bytes);
                                    bitset.reserve_len_exact(max_vendor_id);
                                    for i in 0..=(max_vendor_id % 8) {
                                        if (i as u8) & b > 0 {
                                            bitset.insert((max_vendor_id / 8) + i);
                                        }
                                    }
                                    bitset
                                }
                                None => BitSet::from_bytes(&full_bytes),
                            })
                    )
                )
            >> range_section:
                cond!(
                    encoding_type == Encoding::Range,
                    do_parse!(
                        default_consent: take_bits!(u8, 1) >> num_entries: take_bits!(usize, 12)
                            >> entries:
                                count!(
                                    do_parse!(
                                        entry_type: map!(take_bits!(u8, 1), |x| {
                                            if x == 0 {
                                                EntryType::Single
                                            } else {
                                                EntryType::Range
                                            }
                                        })
                                            >> single_vendor_id:
                                                cond!(
                                                    entry_type == EntryType::Single,
                                                    take_bits!(usize, 16)
                                                )
                                            >> vendor_id_range:
                                                cond!(
                                                    entry_type == EntryType::Range,
                                                    pair!(
                                                        take_bits!(usize, 16),
                                                        take_bits!(usize, 16)
                                                    )
                                                )
                                            >> (match entry_type {
                                                EntryType::Single => {
                                                    Entry::Single(single_vendor_id.unwrap())
                                                }
                                                EntryType::Range => Entry::Range(
                                                    vendor_id_range.unwrap().0,
                                                    vendor_id_range.unwrap().1,
                                                ),
                                            })
                                    ),
                                    num_entries
                                ) >> ({
                            let default_consent = default_consent == 1;
                            let mut vendor_consent =
                                BitVec::from_elem(max_vendor_id, default_consent);
                            for e in entries {
                                match e {
                                    Entry::Single(i) => vendor_consent.set(i - 1, !default_consent),
                                    Entry::Range(start, end) => {
                                        for i in start..=end {
                                            vendor_consent.set(i - 1, !default_consent);
                                        }
                                    }
                                }
                            }

                            BitSet::from_bit_vec(vendor_consent)
                        })
                    )
                ) >> (V1 {
            created: created,
            last_updated: last_updated,
            cmp_id: cmp_id,
            cmp_version: cmp_version,
            consent_screen: consent_screen,
            consent_language: consent_language,
            vendor_list_version: vendor_list_version,
            purposes_allowed: purposes_allowed,
            max_vendor_id: max_vendor_id,
            vendor_consent: match encoding_type {
                Encoding::Bitfield => bitfield_section.unwrap(),
                Encoding::Range => range_section.unwrap(),
            },
        })
    ))
);

pub fn from_str(raw: &str) -> Result<VendorConsent, Error> {
    let bin = base64::decode(raw)?;

    let version = take_version(&bin).to_result()?;
    match version {
        1 => parse_v1(&bin)
            .map(VendorConsent::V1)
            .to_result()
            .map_err(From::from),
        v => Err(Error::UnsupportedVersion(v)),
    }
}

fn serialize_v1(v: V1) -> Result<String, Error> {
    if v.consent_language.len() != 2 {
        return Err(Error::Other(format!(
            "Invalid consent language: {}",
            v.consent_language
        )));
    }

    let language_bytes = v.consent_language.as_bytes();
    for i in 0..=1 {
        if language_bytes[i] < ('a' as u8) || language_bytes[i] > ('z' as u8) {
            return Err(Error::Other(format!(
                "Invalid char '{}' in consent language at position {}",
                language_bytes[i] as char, i
            )));
        }
    }

    // default to true if more than half of bits are set
    let default_consent = v.vendor_consent.len() >= v.max_vendor_id / 2;
    let (range, range_encoded_len) = match default_consent {
        false => create_true_range(&v.vendor_consent),
        true => create_false_range(&v.vendor_consent, v.max_vendor_id),
    };

    // choose smaller encoding
    let encoding_type = if v.max_vendor_id <= range_encoded_len {
        0
    } else {
        1
    };

    let mut raw = Vec::new();
    {
        let mut writer = BitWriter::<BE>::new(&mut raw);
        writer.write(6, 1)?;
        writer.write(36, v.created / 100)?;
        writer.write(36, v.last_updated / 100)?;
        writer.write(12, v.cmp_id)?;
        writer.write(12, v.cmp_version)?;
        writer.write(6, v.consent_screen)?;
        for b in language_bytes {
            writer.write(6, b - ('a' as u8))?;
        }
        writer.write(12, v.vendor_list_version)?;
        writer.write_bytes(&v.purposes_allowed.get_ref().to_bytes())?;
        writer.write(16, v.max_vendor_id as u16)?;
        writer.write(1, encoding_type)?;
        if encoding_type == 0 {
            writer.write_bytes(&v.vendor_consent.get_ref().to_bytes())?;
            writer.byte_align()?;
        } else {
            encode_range(writer, default_consent, range)?;
        }
    }

    Ok(base64::encode_config(&raw, base64::URL_SAFE_NO_PAD))
}

fn create_true_range(vendor_consent: &BitSet) -> (Vec<Entry>, usize) {
    let mut range = Vec::new();
    let mut count = 13; // 1 + 12

    let mut start = None;
    let mut end = None;

    // iterate over set (true) values
    for i in vendor_consent.iter() {
        if start.is_none() {
            start = Some(i);
            end = Some(i);
        } else if i == end.unwrap() + 1 {
            end = Some(i);
        } else {
            if start == end {
                range.push(Entry::Single(start.unwrap() + 1));
                count += 17; // 1 + 16
            } else {
                range.push(Entry::Range(start.unwrap() + 1, end.unwrap() + 1));
                count += 33; // 1 + 16 + 16
            }
            start = None;
            end = None;
        }
    }

    // close range if open
    if !start.is_none() {
        if start == end {
            range.push(Entry::Single(start.unwrap() + 1));
            count += 17; // 1 + 16
        } else {
            range.push(Entry::Range(start.unwrap() + 1, end.unwrap() + 1));
            count += 33; // 1 + 16 + 16
        }
    }

    (range, count)
}

fn create_false_range(vendor_consent: &BitSet, max_vendor_id: usize) -> (Vec<Entry>, usize) {
    let mut inverse = BitSet::from_bit_vec(BitVec::from_elem(max_vendor_id, true));
    inverse.difference_with(vendor_consent);
    create_true_range(&inverse)
}

fn encode_range(mut writer: BitWriter<BE>, default_consent: bool, range: Vec<Entry>) -> Result<(), Error> {
    writer.write_bit(default_consent)?;
    writer.write(12, range.len() as u16)?;

    for e in range {
        match e {
            Entry::Single(x) => {
                writer.write(1, 0)?;
                writer.write(16, x as u16)?;
            }
            Entry::Range(s, e) => {
                writer.write(1, 1)?;
                writer.write(16, s as u16)?;
                writer.write(16, e as u16)?;
            }
        }
    }

    writer.byte_align()?;

    Ok(())
}

pub fn to_str(v: VendorConsent) -> Result<String, Error> {
    match v {
        VendorConsent::V1(v1) => serialize_v1(v1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_good() {
        let mut vendor_consent = BitVec::from_elem(2011, true);
        vendor_consent.set(8, false);
        let vendor_consent = BitSet::from_bit_vec(vendor_consent);

        let v = VendorConsent::V1(V1 {
            created: 1510081144900,
            last_updated: 1510081144900,
            cmp_id: 7,
            cmp_version: 1,
            consent_screen: 3,
            consent_language: "en".to_string(),
            vendor_list_version: 8,
            purposes_allowed: BitSet::from_bytes(&[0b11100000, 0b00000000, 0b00000000]),
            max_vendor_id: 2011,
            vendor_consent: vendor_consent,
        });

        let serialized = to_str(v).unwrap();
        let expected = "BOEFBi5OEFBi5AHABDENAI4AAAB9vABAASA";
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_good() {
        let serialized = "BOEFBi5OEFBi5AHABDENAI4AAAB9vABAASA";
        let v = from_str(serialized).unwrap();

        let expected_purposes_allowed = BitSet::from_bytes(&[0b11100000, 0b00000000, 0b00000000]);

        let expected_max_vendor_id = 2011;
        let mut expected_vendor_consent = BitVec::from_elem(expected_max_vendor_id, true);
        expected_vendor_consent.set(8, false);
        let expected_vendor_consent = BitSet::from_bit_vec(expected_vendor_consent);

        match v {
            VendorConsent::V1(v1) => {
                assert_eq!(v1.created, 1510081144900);
                assert_eq!(v1.last_updated, 1510081144900);
                assert_eq!(v1.cmp_id, 7);
                assert_eq!(v1.cmp_version, 1);
                assert_eq!(v1.consent_screen, 3);
                assert_eq!(v1.consent_language, "en");
                assert_eq!(v1.vendor_list_version, 8);
                assert_eq!(v1.purposes_allowed, expected_purposes_allowed);
                assert_eq!(v1.max_vendor_id, expected_max_vendor_id);
                assert_eq!(v1.vendor_consent, expected_vendor_consent);
            }
        }
    }
}
