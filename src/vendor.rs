use std::convert::From;

use base64;
use bit_set::BitSet;
use bit_vec::BitVec;
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

    // For each vendor id listend in the global vendor list, the presence indicates consent.
    pub vendor_consent: BitSet,
}

pub enum VendorConsent {
    V1(V1),
}

#[derive(Debug, PartialEq)]
pub enum Error {
    Base64DecodeError(base64::DecodeError),
    UnsupportedVersion(u8),
    Other(String),
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

named!( take_version<u8>, bits!( take_bits!( u8, 6 ) ) );

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


named!( parse_v1<V1>,
    bits!(do_parse!(
        version: tag_bits!(u8, 6, 1)
    >>  created: map!( take_bits!( u64, 36 ), |x| x*100 )
    >>  last_updated: map!( take_bits!( u64, 36 ), |x| x*100 )
    >>  cmp_id: take_bits!( u16, 12 ) 
    >>  cmp_version: take_bits!( u16, 12 ) 
    >>  consent_screen: take_bits!( u8, 6 ) 
    >>  consent_language: map!( pair!( take_bits!( u8, 6 ), take_bits!( u8, 6 ) ), consent_language )
    >>  vendor_list_version: take_bits!( u16, 12 ) 
    >>  purposes_allowed: map!( count!( take_bits!(u8, 8), 3 ), |x| BitSet::from_bytes(&x) )
    >>  max_vendor_id: take_bits!( usize, 16 ) 
    >>  encoding_type: map!( take_bits!( u8, 1 ), |x| if x == 0 { Encoding::Bitfield } else { Encoding::Range } )
    >>  bitfield_section: cond!( encoding_type == Encoding::Bitfield, do_parse!(
            full_bytes: count!( take_bits!( u8, 8 ), max_vendor_id / 8 )
        >>  leftover_byte: cond!( max_vendor_id % 8 > 0, take_bits!( u8, max_vendor_id % 8 ) )
        >>  ( match leftover_byte {
                Some(b) => {
                    let mut bitset = BitSet::from_bytes(&full_bytes);
                    bitset.reserve_len_exact(max_vendor_id);
                    for i in 0..=(max_vendor_id % 8) {
                        if (i as u8) & b > 0 {
                            bitset.insert((max_vendor_id / 8) + i);
                        }
                    }
                    bitset
                },
                None => BitSet::from_bytes(&full_bytes),
        })))
    >>  range_section: cond!( encoding_type == Encoding::Range, do_parse!(
            default_consent: take_bits!( u8, 1 )
        >>  num_entries: take_bits!( usize, 12 )
        >>  entries: count!( do_parse!(
                entry_type: map!( take_bits!( u8, 1 ), |x| if x == 0 { EntryType::Single } else { EntryType::Range } )
            >>  single_vendor_id: cond!( entry_type == EntryType::Single, take_bits!( usize, 16 ) )
            >>  vendor_id_range: cond!( entry_type == EntryType::Range, pair!( take_bits!( usize, 16 ), take_bits!( usize, 16 ) ) )
            >> ( match entry_type {
                    EntryType::Single => Entry::Single(single_vendor_id.unwrap()),
                    EntryType::Range => Entry::Range(vendor_id_range.unwrap().0, vendor_id_range.unwrap().1),
            })), num_entries )
        >>  ({
            let default_consent = default_consent == 1;
            let mut vendor_consent = BitVec::from_elem(max_vendor_id, default_consent);
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
        })))
    >> (V1 {
        created: created,
        last_updated: last_updated,
        cmp_id: cmp_id,
        cmp_version: cmp_version,
        consent_screen: consent_screen,
        consent_language: consent_language, 
        vendor_list_version: vendor_list_version,
        purposes_allowed: purposes_allowed,
        vendor_consent: match encoding_type {
            Encoding::Bitfield => bitfield_section.unwrap(),
            Encoding::Range => range_section.unwrap(),
        },
    })))
);

const LETTER_OFFSET: u8 = 'a' as u8;
fn consent_language(letters: (u8, u8)) -> String {
    String::from_utf8(
        vec![letters.0 + LETTER_OFFSET,
             letters.1 + LETTER_OFFSET]).unwrap()
}

pub fn from_str(raw: &str) -> Result<VendorConsent, Error> {
    let bin = base64::decode(raw)?;

    let version = take_version(&bin).to_result()?;
    match version {
        1 => parse_v1(&bin).map(VendorConsent::V1).to_result().map_err(From::from),
        v => Err(Error::UnsupportedVersion(v)),
    }
}

pub fn to_str(v: VendorConsent) -> String {
    let mut raw = Vec::new();
    
    match v {
        VendorConsent::V1(_) => raw.push(1),
    }

    base64::encode(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_good() {
        let v = VendorConsent::V1(
            V1 {
                created: 1510081144900,
                last_updated: 1510081144900,
                cmp_id: 7,
                cmp_version: 1,
                consent_screen: 3,
                consent_language: "en".to_string(), 
                vendor_list_version: 8,
                purposes_allowed: BitSet::from_bytes(&[0b11100000, 0b00000000, 0b00000000]),
                vendor_consent: BitSet::with_capacity(0),
            }
        );

        let serialized = to_str(v);
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
                assert_eq!(v1.vendor_consent, expected_vendor_consent);
            }
        }
    }
}
