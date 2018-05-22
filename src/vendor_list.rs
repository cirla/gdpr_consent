// Copyright (c) 2018 The gdpr_consent authors
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cmp;
use std::collections::HashMap;
use std::convert::From;
use std::error;
use std::fmt::{self, Display};
use std::hash;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde;
use serde::ser::SerializeSeq;
use serde_json;

trait HasId<T> {
    fn id(&self) -> T;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Vendor {
    pub id: u16,
    pub name: String,
    #[serde(rename = "policyUrl")]
    pub policy_url: String,
    #[serde(rename = "purposeIds")]
    pub purpose_ids: Vec<u8>,
    #[serde(rename = "legIntPurposeIds")]
    pub leg_int_purpose_ids: Vec<u8>,
    #[serde(rename = "featureIds")]
    pub feature_ids: Vec<u8>,
}

impl HasId<u16> for Vendor {
    fn id(&self) -> u16 {
        self.id
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Purpose {
    pub id: u8,
    pub name: String,
    pub description: String,
}

impl HasId<u8> for Purpose {
    fn id(&self) -> u8 {
        self.id
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Feature {
    pub id: u8,
    pub name: String,
    pub description: String,
}

impl HasId<u8> for Feature {
    fn id(&self) -> u8 {
        self.id
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VendorList {
    #[serde(rename = "vendorListVersion")]
    pub version: u16,

    #[serde(rename = "lastUpdated")]
    pub last_updated: DateTime<Utc>,

    #[serde(
        serialize_with = "serialize_id_map_as_list", deserialize_with = "deserialize_list_as_id_map"
    )]
    pub purposes: HashMap<u8, Purpose>,

    #[serde(
        serialize_with = "serialize_id_map_as_list", deserialize_with = "deserialize_list_as_id_map"
    )]
    pub features: HashMap<u8, Feature>,

    #[serde(
        serialize_with = "serialize_id_map_as_list", deserialize_with = "deserialize_list_as_id_map"
    )]
    pub vendors: HashMap<u16, Vendor>,
}

impl VendorList {
    pub fn to_string(&self) -> Result<String, Error> {
        serde_json::to_string(self).map_err(From::from)
    }
}

impl FromStr for VendorList {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(From::from)
    }
}

#[derive(Debug)]
pub enum Error {
    JsonError(serde_json::Error),
    Other(String),
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::JsonError(ref err) => err.description(),
            Error::Other(msg) => msg,
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            Error::JsonError(ref err) => Some(err),
            Error::Other(_) => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::JsonError(ref err) => Display::fmt(err, f),
            Error::Other(msg) => Display::fmt(msg, f),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::JsonError(e)
    }
}

fn deserialize_list_as_id_map<'de, D, K, V>(deserializer: D) -> Result<HashMap<K, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    K: Eq + hash::Hash,
    V: HasId<K> + serde::Deserialize<'de>,
{
    let values: Vec<V> = serde::Deserialize::deserialize(deserializer)?;
    Ok(values.into_iter().map(|v| (v.id(), v)).collect())
}

fn serialize_id_map_as_list<S, K, V>(map: &HashMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
    K: Eq + hash::Hash + cmp::Ord,
    V: HasId<K> + serde::Serialize,
{
    let mut values: Vec<&V> = map.iter().map(|(_, v)| v).collect();
    values.sort_by_key(|v| v.id());

    let mut seq = serializer.serialize_seq(Some(values.len()))?;
    for element in values.iter() {
        seq.serialize_element(element)?;
    }
    seq.end()
}

#[cfg(test)]
mod tests {
    #[test]
    fn serialize_good() {}

    #[test]
    fn deserialize_good() {}
}
