use lorawan_encoding::keys::AES128;

pub type AppEui = [u8; 8];
pub type DevEui = [u8; 8];

#[derive(Debug)]
pub struct Credentials {
    deveui: DevEui,
    appeui: AppEui,
    appkey: AES128,
}

impl Credentials {
    pub fn new(appeui: AppEui, deveui: DevEui, appkey: [u8; 16]) -> Credentials {
        Credentials {
            deveui,
            appeui,
            appkey: appkey.into(),
        }
    }

    pub fn appeui(&self) -> &AppEui {
        &self.appeui
    }

    pub fn deveui(&self) -> &DevEui {
        &self.deveui
    }

    pub fn appkey(&self) -> &AES128 {
        &self.appkey
    }
}
