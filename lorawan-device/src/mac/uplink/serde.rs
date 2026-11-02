use crate::mac::uplink::{MAX_MAC_COMMAND_LEN, Uplink};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[cfg(feature = "serde")]
impl Serialize for Uplink {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Uplink", 2)?;
        state.serialize_field("confirmed", &self.confirmed)?;
        state.serialize_field("pending_data", self.pending.as_slice())?;
        state.end()
    }
}

#[cfg(feature = "serde")]
struct PendingData(heapless::Vec<u8, MAX_MAC_COMMAND_LEN>);

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for PendingData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PendingDataVisitor;

        impl<'de> serde::de::Visitor<'de> for PendingDataVisitor {
            type Value = PendingData;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("a sequence of MAC command bytes")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<PendingData, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut data = heapless::Vec::new();
                while let Some(byte) = seq.next_element::<u8>()? {
                    data.push(byte).map_err(|_| {
                        serde::de::Error::custom("pending_data exceeds maximum size")
                    })?;
                }
                Ok(PendingData(data))
            }
        }

        deserializer.deserialize_seq(PendingDataVisitor)
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
                let mut pending_data: Option<PendingData> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Confirmed => {
                            if confirmed.is_some() {
                                return Err(de::Error::duplicate_field("confirmed"));
                            }
                            confirmed = Some(map.next_value()?);
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
                let pending_data =
                    pending_data.ok_or_else(|| de::Error::missing_field("pending_data"))?;

                Ok(Uplink { pending: pending_data.0, confirmed })
            }
        }

        deserializer.deserialize_struct("Uplink", &["confirmed", "pending_data"], UplinkVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lorawan::packet_length::phy::mac::fhdr::FOPTS_MAX_LEN;

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

    #[test]
    fn test_serde_large_pending() {
        let mut uplink = Uplink::default();
        let data = [42u8; 64];
        uplink.pending.extend_from_slice(&data).unwrap();

        let json = serde_json::to_string(&uplink).unwrap();
        let decoded: Uplink = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.mac_commands(), &data);
    }
}
