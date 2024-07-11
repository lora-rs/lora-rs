// Copyright (c) 2017,2018,2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

//! This module implements LoRaWAN packet handling and parsing.
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::upper_case_acronyms)]
#![doc = include_str!("../README.md")]

pub mod creator;
pub mod keys;
pub mod maccommandcreator;
pub mod maccommands;
pub mod packet_length;
pub mod parser;
pub mod string;

#[cfg(feature = "full")]
pub mod extra;

#[cfg(feature = "default-crypto")]
#[cfg_attr(docsrs, doc(cfg(feature = "default-crypto")))]
pub mod default_crypto;

mod securityhelpers;
