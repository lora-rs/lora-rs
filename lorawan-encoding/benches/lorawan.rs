// Copyright (c) 2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

use criterion::{Criterion, criterion_group, criterion_main};
use lorawan::default_crypto::DefaultCrypto;
use lorawan::maccommands::DownlinkMacCommand;
use lorawan::maccommands::parse_downlink_mac_commands;
use std::alloc::System;
use std::sync::atomic::{AtomicUsize, Ordering};

extern crate std;

use lorawan::keys::*;
use lorawan::parser::{DecryptedDataPayload, FrmPayload, PhyPayload, parse};

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
    let crypto = DefaultCrypto::new(&AES128([2; 16]));
    let cnt = AtomicUsize::new(0);
    GLOBAL.usage();
    c.bench_function("data_payload_mic_validation", |b| {
        b.iter(|| {
            cnt.fetch_add(1usize, Ordering::SeqCst);
            let data = data_payload();
            let phy = parse(&data).unwrap();

            if let PhyPayload::Data(data_payload) = phy {
                assert!(data_payload.validate_mic(&crypto, 1));
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
    let crypto = DefaultCrypto::new(AppSKey::from([1; 16]).inner());
    let cnt = AtomicUsize::new(0);
    GLOBAL.usage();
    c.bench_function("data_payload_decrypt", |b| {
        b.iter(|| {
            cnt.fetch_add(1usize, Ordering::SeqCst);
            let mut data = data_payload();
            let dec =
                DecryptedDataPayload::decrypt_in_place(&mut data, None, Some(&crypto), 1).unwrap();
            assert_eq!(dec.frm_payload(), FrmPayload::Data(&payload[..]));
        })
    });
    let n = cnt.load(Ordering::SeqCst);
    println!("Approximate memory usage per iteration: {} from {}", GLOBAL.usage() / n, n);
}

fn bench_complete_data_payload_creation(c: &mut Criterion) {
    use core::num::NonZeroU8;
    use lorawan::creator::{DataFrame, Payload};
    use lorawan::parser::{DataFrameType, DevAddr};

    let nwk_crypto = DefaultCrypto::new(&AES128([2; 16]));
    let app_crypto = DefaultCrypto::new(&AES128([1; 16]));
    let cnt = AtomicUsize::new(0);
    GLOBAL.usage();
    c.bench_function("data_payload_creation", |b| {
        b.iter(|| {
            cnt.fetch_add(1usize, Ordering::SeqCst);
            let frame = DataFrame {
                frame_type: DataFrameType::UnconfirmedUp,
                dev_addr: DevAddr::from_value(0x01020304),
                fcnt: 76543,
                payload: Payload::Data { f_port: NonZeroU8::new(42).unwrap(), data: b"hello lora" },
                ..Default::default()
            };
            let mut buf = [0u8; 64];
            frame.build_into(&mut buf, &nwk_crypto, Some(&app_crypto)).unwrap();
        })
    });
    let n = cnt.load(Ordering::SeqCst);
    println!("Approximate memory usage per iteration: {} from {}", GLOBAL.usage() / n, n);
}

criterion_group!(
    benches,
    bench_complete_data_payload_fhdr,
    bench_complete_data_payload_mic_validation,
    bench_complete_data_payload_decrypt,
    bench_complete_data_payload_creation
);
criterion_main!(benches);

fn data_payload() -> [u8; 18] {
    [
        0x40, 0x04, 0x03, 0x02, 0x01, 0x80, 0x01, 0x00, 0x01, 0xa6, 0x94, 0x64, 0x26, 0x15, 0xd6,
        0xc3, 0xb5, 0x82,
    ]
}
