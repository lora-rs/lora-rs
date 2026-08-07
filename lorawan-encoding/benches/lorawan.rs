// Copyright (c) 2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

use crate::CryptoFactory;
use aes::cipher::KeyInit;
use aes::Aes128;
use criterion::{criterion_group, criterion_main, Criterion};
use lorawan::maccommands::parse_downlink_mac_commands;
use lorawan::maccommands::DownlinkMacCommand;
use std::alloc::System;
use std::sync::atomic::{AtomicUsize, Ordering};

extern crate std;

use lorawan::keys::*;
use lorawan::parser::{parse, DecryptedDataPayload, FrmPayload, PhyPayload};

#[global_allocator]
static GLOBAL: trallocator::Trallocator<System> = trallocator::Trallocator::new(System);

fn bench_complete_data_payload_fhdr(c: &mut Criterion) {
    let cnt = AtomicUsize::new(0);
    GLOBAL.usage();
    c.bench_function("data_payload_headers_parsing", |b| {
        b.iter(|| {
            cnt.fetch_add(1usize, Ordering::SeqCst);
            let data = data_payload();
            let phy = parse(&data).unwrap();

            if let PhyPayload::Data(data_payload) = phy {
                assert!(data_payload.is_uplink());
                assert!(!data_payload.is_confirmed());
                if data_payload.mic().0[0] < 1 {
                    panic!("no way");
                }

                let fhdr = data_payload.fhdr();

                if fhdr.dev_addr().value() < 1 {
                    panic!("no way");
                }
                assert_eq!(fhdr.fcnt(), 1u16);
                assert_eq!(
                    parse_downlink_mac_commands(fhdr.f_opts())
                        .filter_map(|c: Result<DownlinkMacCommand<'_>, _>| c.ok())
                        .count(),
                    0
                );

                let fctrl = fhdr.fctrl();

                assert_eq!(fctrl.f_opts_len(), 0);

                assert!(!fctrl.f_pending(), "no f_pending");

                assert!(!fctrl.ack(), "no ack");

                assert!(fctrl.adr(), "ADR");
            } else {
                panic!("failed to parse DataPayload");
            }
        })
    });
    let n = cnt.load(Ordering::SeqCst);
    println!("Approximate memory usage per iteration: {} from {}", GLOBAL.usage() / n, n);
}

fn bench_complete_data_payload_mic_validation(c: &mut Criterion) {
    let mic_key = AES128([2; 16]);
    let factory = ConstFactory::new(&mic_key);
    let cnt = AtomicUsize::new(0);
    GLOBAL.usage();
    c.bench_function("data_payload_mic_validation", |b| {
        b.iter(|| {
            cnt.fetch_add(1usize, Ordering::SeqCst);
            let data = data_payload();
            let phy = parse(&data).unwrap();

            if let PhyPayload::Data(data_payload) = phy {
                assert!(data_payload.validate_mic(&mic_key, 1, &factory));
            } else {
                panic!("failed to parse DataPayload");
            }
        })
    });
    let n = cnt.load(Ordering::SeqCst);
    println!("Approximate memory usage per iteration: {} from {}", GLOBAL.usage() / n, n);
}

fn bench_complete_data_payload_decrypt(c: &mut Criterion) {
    let mut payload = Vec::new();
    payload.extend_from_slice(&String::from("hello").into_bytes()[..]);
    let key = AppSKey::from([1; 16]);
    let factory = ConstFactory::new(key.inner());
    let cnt = AtomicUsize::new(0);
    GLOBAL.usage();
    c.bench_function("data_payload_decrypt", |b| {
        b.iter(|| {
            cnt.fetch_add(1usize, Ordering::SeqCst);
            let mut data = data_payload();
            let dec =
                DecryptedDataPayload::decrypt_in_place(&mut data, None, Some(&key), 1, &factory)
                    .unwrap();
            assert_eq!(dec.frm_payload(), FrmPayload::Data(&payload[..]));
        })
    });
    let n = cnt.load(Ordering::SeqCst);
    println!("Approximate memory usage per iteration: {} from {}", GLOBAL.usage() / n, n);
}

pub type Cmac = cmac::Cmac<Aes128>;

#[derive(Debug, Clone)]
pub struct ConstFactory(Aes128, Cmac);

impl PartialEq for ConstFactory {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl ConstFactory {
    fn new(key: &AES128) -> Self {
        ConstFactory(
            Aes128::new_from_slice(&key.0[..]).unwrap(),
            Cmac::new_from_slice(&key.0[..]).unwrap(),
        )
    }
}

impl CryptoFactory for ConstFactory {
    type E = Aes128;
    type D = Aes128;
    type M = Cmac;

    fn new_enc(&self, _: &AES128) -> Self::E {
        self.0.clone()
    }

    fn new_dec(&self, _: &AES128) -> Self::D {
        self.0.clone()
    }

    fn new_mac(&self, _: &AES128) -> Self::M {
        self.1.clone()
    }
}

criterion_group!(
    benches,
    bench_complete_data_payload_fhdr,
    bench_complete_data_payload_mic_validation,
    bench_complete_data_payload_decrypt
);
criterion_main!(benches);

fn data_payload() -> [u8; 18] {
    [
        0x40, 0x04, 0x03, 0x02, 0x01, 0x80, 0x01, 0x00, 0x01, 0xa6, 0x94, 0x64, 0x26, 0x15, 0xd6,
        0xc3, 0xb5, 0x82,
    ]
}
