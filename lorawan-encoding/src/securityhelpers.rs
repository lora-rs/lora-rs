use super::keys::{Crypto, MIC};

/// calculate_data_mic computes the MIC of a correct data packet.
pub fn calculate_data_mic(data: &[u8], crypto: &dyn Crypto, fcnt: u32) -> MIC {
    let mut b0 = [0; 16];

    // compute b0 from the spec
    generate_helper_block(data, 0x49, fcnt, &mut b0[..16]);
    b0[15] = data.len() as u8;

    MIC(crypto.calculate_mic(&b0[..], data))
}

fn generate_helper_block(data: &[u8], first: u8, fcnt: u32, res: &mut [u8]) {
    res[0] = first;
    // res[1..5] are 0
    res[5] = (data[0] & 0x20) >> 5;
    res[6..10].copy_from_slice(&data[1..5]);
    // fcnt
    res[10] = (fcnt & 0xff) as u8;
    res[11] = ((fcnt >> 8) & 0xff) as u8;
    res[12] = ((fcnt >> 16) & 0xff) as u8;
    res[13] = ((fcnt >> 24) & 0xff) as u8;
    // res[14] is 0
    // res[15] is to be set later
}

/// calculate_mic computes the MIC of a correct data packet.
pub fn calculate_mic(data: &[u8], crypto: &dyn Crypto) -> MIC {
    MIC(crypto.calculate_mic(&[], data))
}

/// encrypt_frm_data_payload encrypts bytes
pub fn encrypt_frm_data_payload(
    phy_payload: &mut [u8],
    start: usize,
    end: usize,
    fcnt: u32,
    crypto: &dyn Crypto,
) {
    let len = end - start;

    let mut a = [0u8; 16];
    generate_helper_block(phy_payload, 0x01, fcnt, &mut a[..]);

    let mut s = [0u8; 16];

    let mut ctr = 1;
    for i in 0..len {
        let j = i & 0x0f;
        if j == 0 {
            a[15] = ctr;
            ctr += 1;
            s = a;
            crypto.encrypt_block(&mut s);
        }
        phy_payload[start + i] ^= s[j]
    }
}
