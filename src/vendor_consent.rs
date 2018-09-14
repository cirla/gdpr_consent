// Copyright (c) 2018 The gdpr_consent authors
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::convert::From;
use std::error;
use std::fmt::{self, Display};
use std::io;
use std::str::FromStr;
use std::string;

use base64;
use bit_set::BitSet;
use bit_vec::BitVec;
use bitstream_io::{BigEndian, BitReader, BitWriter};
use chrono::{DateTime, TimeZone, Utc};

#[derive(Debug, PartialEq)]
pub struct V1 {
    // Epoch ms when consent string was first created
    pub created: DateTime<Utc>,

    // Epoch ms when consent string was last updated
    pub last_updated: DateTime<Utc>,

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

impl VendorConsent {
    pub fn to_string(&self) -> Result<String, Error> {
        match self {
            VendorConsent::V1(ref v1) => serialize_v1(v1),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Base64DecodeError(base64::DecodeError),
    UnsupportedVersion(u8),
    IoError(io::Error),
    FromUtf8Error(string::FromUtf8Error),
    Other(String),
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::Base64DecodeError(ref err) => err.description(),
            Error::UnsupportedVersion(_) => "Unsupported version",
            Error::IoError(ref err) => err.description(),
            Error::FromUtf8Error(ref err) => err.description(),
            Error::Other(msg) => msg,
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            Error::Base64DecodeError(ref err) => Some(err),
            Error::UnsupportedVersion(_) => None,
            Error::IoError(ref err) => Some(err),
            Error::FromUtf8Error(ref err) => Some(err),
            Error::Other(_) => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Base64DecodeError(ref err) => Display::fmt(err, f),
            Error::UnsupportedVersion(v) => write!(f, "Unsupported version: {}", v),
            Error::IoError(ref err) => Display::fmt(err, f),
            Error::FromUtf8Error(ref err) => Display::fmt(err, f),
            Error::Other(msg) => Display::fmt(msg, f),
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IoError(e)
    }
}

impl From<string::FromUtf8Error> for Error {
    fn from(e: string::FromUtf8Error) -> Error {
        Error::FromUtf8Error(e)
    }
}

impl From<base64::DecodeError> for Error {
    fn from(e: base64::DecodeError) -> Error {
        Error::Base64DecodeError(e)
    }
}

#[derive(Debug, PartialEq)]
enum Entry {
    Single(usize),
    Range(usize, usize),
}

fn parse_v1_bitfield<R>(
    mut reader: BitReader<R, BigEndian>,
    max_vendor_id: usize,
) -> Result<BitSet, Error>
where R: io::Read
{
    let buf_size = max_vendor_id / 8 + if max_vendor_id % 8 == 0 { 0 } else { 1 };
    let mut buf = Vec::with_capacity(buf_size);

    // read full bytes
    for _ in 0..buf_size {
        buf.push(reader.read::<u8>(8)?);
    }

    // read remainder
    if max_vendor_id % 8 > 0 {
        buf.push(reader.read::<u8>(max_vendor_id as u32 % 8)?);
    }

    Ok(BitSet::from_bytes(&buf))
}

fn parse_v1_range<R>(
    mut reader: BitReader<R, BigEndian>,
    max_vendor_id: usize,
) -> Result<BitSet, Error>
where R: io::Read
{
    let default_consent = reader.read::<u8>(1)? == 1;
    let num_entries = reader.read::<u16>(12)? as usize;

    let mut buf = BitVec::from_elem(max_vendor_id, default_consent);
    for _ in 0..num_entries {
        match reader.read::<u8>(1)? {
            0 => {
                let id = reader.read::<u16>(16)? as usize;
                buf.set(id - 1, !default_consent)
            }
            _ => {
                let start = reader.read::<u16>(16)? as usize;
                let end = reader.read::<u16>(16)? as usize;
                for id in start..=end {
                    buf.set(id - 1, !default_consent);
                }
            }
        }
    }

    Ok(BitSet::from_bit_vec(buf))
}

const DECISECS_IN_SEC: i64 = 10;
const MILLISECS_IN_DECISEC: u32 = 100;
const NANOSECS_IN_DECISEC: u32 = 100_000_000;

fn parse_v1<R>(mut reader: BitReader<R, BigEndian>) -> Result<V1, Error>
where R: io::Read
{
    let created = reader.read::<i64>(36)?;
    let created = Utc.timestamp(
        created / DECISECS_IN_SEC,
        (created % DECISECS_IN_SEC) as u32 * NANOSECS_IN_DECISEC,
    );
    let last_updated = reader.read::<i64>(36)?;
    let last_updated = Utc.timestamp(
        last_updated / DECISECS_IN_SEC,
        (last_updated % DECISECS_IN_SEC) as u32 * NANOSECS_IN_DECISEC,
    );
    let cmp_id = reader.read::<u16>(12)?;
    let cmp_version = reader.read::<u16>(12)?;
    let consent_screen = reader.read::<u8>(6)?;

    let mut buf = Vec::with_capacity(2);
    for _ in 0..2 {
        buf.push(reader.read::<u8>(6)? + 'a' as u8);
    }
    let consent_language = String::from_utf8(buf)?;

    let vendor_list_version = reader.read::<u16>(12)?;

    let mut buf: [u8; 3] = Default::default();
    reader.read_bytes(&mut buf)?;
    let purposes_allowed = BitSet::from_bytes(&buf);

    let max_vendor_id = reader.read::<u16>(16)? as usize;

    let vendor_consent = match reader.read::<u8>(1)? {
        0 => parse_v1_bitfield(reader, max_vendor_id)?,
        _ => parse_v1_range(reader, max_vendor_id)?,
    };

    Ok(V1 {
        created: created,
        last_updated: last_updated,
        cmp_id: cmp_id,
        cmp_version: cmp_version,
        consent_screen: consent_screen,
        consent_language: consent_language,
        vendor_list_version: vendor_list_version,
        purposes_allowed: purposes_allowed,
        max_vendor_id: max_vendor_id,
        vendor_consent: vendor_consent,
    })
}

impl FromStr for VendorConsent {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let data = base64::decode(s)?;
        let mut cursor = io::Cursor::new(&data);
        let mut reader = BitReader::endian(&mut cursor, BigEndian);

        let version = reader.read::<u8>(6)?;
        match version {
            1 => parse_v1(reader).map(VendorConsent::V1),
            v => Err(Error::UnsupportedVersion(v)),
        }
    }
}

fn serialize_v1(v: &V1) -> Result<String, Error> {
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
        let mut writer = BitWriter::endian(&mut raw, BigEndian);
        writer.write(6, 1)?;
        writer.write(
            36,
            v.created.timestamp() * 10
                + (v.created.timestamp_subsec_millis() / MILLISECS_IN_DECISEC) as i64,
        )?;
        writer.write(
            36,
            v.last_updated.timestamp() * 10
                + (v.last_updated.timestamp_subsec_millis() / MILLISECS_IN_DECISEC) as i64,
        )?;
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
            start = Some(i);
            end = Some(i);
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

fn encode_range<W>(
    mut writer: BitWriter<W, BigEndian>,
    default_consent: bool,
    range: Vec<Entry>,
) -> Result<(), Error>
where W: io::Write
{
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_good() {
        let mut vendor_consent = BitVec::from_elem(2011, true);
        vendor_consent.set(8, false);
        let vendor_consent = BitSet::from_bit_vec(vendor_consent);

        let v = VendorConsent::V1(V1 {
            created: "2017-11-07T19:15:55.4Z".parse().unwrap(),
            last_updated: "2017-11-07T19:15:55.4Z".parse().unwrap(),
            cmp_id: 7,
            cmp_version: 1,
            consent_screen: 3,
            consent_language: "en".to_string(),
            vendor_list_version: 8,
            purposes_allowed: BitSet::from_bytes(&[0b11100000, 0b00000000, 0b00000000]),
            max_vendor_id: 2011,
            vendor_consent: vendor_consent,
        });

        let serialized = v.to_string().unwrap();
        let expected = "BOEFEAyOEFEAyAHABDENAI4AAAB9vABAASA";
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_good() {
        let v = "BOEFEAyOEFEAyAHABDENAI4AAAB9vABAASA".parse().unwrap();

        let expected_purposes_allowed = BitSet::from_bytes(&[0b11100000, 0b00000000, 0b00000000]);

        let expected_max_vendor_id = 2011;
        let mut expected_vendor_consent = BitVec::from_elem(expected_max_vendor_id, true);
        expected_vendor_consent.set(8, false);
        let expected_vendor_consent = BitSet::from_bit_vec(expected_vendor_consent);

        match v {
            VendorConsent::V1(v1) => {
                assert_eq!(Ok(v1.created), "2017-11-07T19:15:55.4Z".parse());
                assert_eq!(Ok(v1.last_updated), "2017-11-07T19:15:55.4Z".parse());
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
