use crate::mac::uplink::Uplink;
use lorawan::packet_length::phy::mac::fhdr::FOPTS_MAX_LEN;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[cfg(feature = "serde")]
impl Serialize for Uplink {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Uplink", 3)?;
        state.serialize_field("confirmed", &self.confirmed)?;
        state.serialize_field("pending_len", &(self.pending.len() as u8))?;
        let mut full_array = [0u8; FOPTS_MAX_LEN];
        full_array[..self.pending.len()].copy_from_slice(&self.pending);
        state.serialize_field("pending_data", &full_array)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Uplink {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use core::fmt;
        use serde::de::{self, MapAccess, Visitor};

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Confirmed,
            PendingLen,
            PendingData,
        }

        struct UplinkVisitor;

        impl<'de> Visitor<'de> for UplinkVisitor {
            type Value = Uplink;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("struct Uplink")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Uplink, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut confirmed: Option<bool> = None;
                let mut pending_len: Option<u8> = None;
                let mut pending_data: Option<[u8; FOPTS_MAX_LEN]> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Confirmed => {
                            if confirmed.is_some() {
                                return Err(de::Error::duplicate_field("confirmed"));
                            }
                            confirmed = Some(map.next_value()?);
                        }
                        Field::PendingLen => {
                            if pending_len.is_some() {
                                return Err(de::Error::duplicate_field("pending_len"));
                            }
                            pending_len = Some(map.next_value()?);
                        }
                        Field::PendingData => {
                            if pending_data.is_some() {
                                return Err(de::Error::duplicate_field("pending_data"));
                            }
                            pending_data = Some(map.next_value()?);
                        }
                    }
                }

                let confirmed = confirmed.ok_or_else(|| de::Error::missing_field("confirmed"))?;
                let pending_len =
                    pending_len.ok_or_else(|| de::Error::missing_field("pending_len"))?;
                let pending_data =
                    pending_data.ok_or_else(|| de::Error::missing_field("pending_data"))?;

                if pending_len as usize > FOPTS_MAX_LEN {
                    return Err(de::Error::custom("pending_len exceeds maximum size"));
                }

                let mut pending = heapless::Vec::new();
                pending
                    .extend_from_slice(&pending_data[..pending_len as usize])
                    .map_err(|_| de::Error::custom("failed to create heapless::Vec"))?;

                Ok(Uplink { pending, confirmed })
            }
        }

        deserializer.deserialize_struct(
            "Uplink",
            &["confirmed", "pending_len", "pending_data"],
            UplinkVisitor,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_empty_uplink() {
        let uplink = Uplink::default();
        let json = serde_json::to_string(&uplink).unwrap();
        let decoded: Uplink = serde_json::from_str(&json).unwrap();
        assert!(!decoded.confirms_downlink());
        assert_eq!(decoded.mac_commands(), &[0u8; 0]);
    }

    #[test]
    fn test_serde_uplink_with_data() {
        let mut uplink = Uplink::default();
        uplink.set_downlink_confirmation();
        uplink.pending.extend_from_slice(&[1, 2, 3, 4]).unwrap();

        let json = serde_json::to_string(&uplink).unwrap();
        let decoded: Uplink = serde_json::from_str(&json).unwrap();
        assert!(decoded.confirms_downlink());
        assert_eq!(decoded.mac_commands(), &[1, 2, 3, 4]);
    }

    #[test]
    fn test_serde_max_size() {
        let mut uplink = Uplink::default();
        let max_data = [42u8; FOPTS_MAX_LEN];
        uplink.pending.extend_from_slice(&max_data).unwrap();

        let json = serde_json::to_string(&uplink).unwrap();
        let decoded: Uplink = serde_json::from_str(&json).unwrap();
        assert!(!decoded.confirms_downlink());
        assert_eq!(decoded.mac_commands(), &max_data);
    }
}
