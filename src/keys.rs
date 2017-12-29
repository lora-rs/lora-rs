// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

/// AES128 represents 128 bit AES key.
#[derive(Debug, PartialEq)]
pub struct AES128(pub [u8; 16]);

/// MIC represents LoRaWAN MIC.
#[derive(Debug, PartialEq)]
pub struct MIC(pub [u8; 4]);
