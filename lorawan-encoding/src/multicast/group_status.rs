use crate::maccommands::Error;
use crate::multicast::{
    McGroupStatusAnsPayload, McGroupStatusReqCreator, McGroupStatusReqPayload, MAX_GROUPS,
};
use crate::parser::McAddr;

pub struct McGroupStatusItem<'a>(pub(crate) &'a [u8]);

impl<'a> McGroupStatusItem<'a> {
    /// Creates a new instance of the MAC command if there is enough data.
    pub fn new(data: &'a [u8]) -> Result<McGroupStatusItem<'a>, Error> {
        if data.len() != Self::len() {
            Err(Error::BufferTooShort)
        } else {
            Ok(McGroupStatusItem(data))
        }
    }

    pub const fn len() -> usize {
        5
    }

    pub fn mc_group_id(&self) -> u8 {
        self.0[0]
    }
    pub fn mc_addr(&self) -> McAddr<&'a [u8]> {
        McAddr::new_from_raw(&self.0[1..5])
    }
}
impl<'a> McGroupStatusAnsPayload<'a> {
    const ITEM_LEN: usize = 5;
    pub fn new(data: &'a [u8]) -> Result<McGroupStatusAnsPayload<'a>, Error> {
        if data.is_empty() {
            return Err(Error::BufferTooShort);
        }
        let status = data[0];
        let required_len = Self::required_len(status);
        if data.len() < required_len {
            return Err(Error::BufferTooShort);
        }
        Ok(McGroupStatusAnsPayload(&data[0..required_len]))
    }

    pub fn required_len(status: u8) -> usize {
        // |  RFU  | NbTotalGroups | AnsGroupMask |
        // | 1 bit |    3 bits     |    4 bits    |
        // Table 5: McGroupStatusAns
        let ans_group_mask = status & 0b1111;
        let groups_in_report = ans_group_mask.count_ones();
        groups_in_report as usize * McGroupStatusItem::len()
    }

    /// AnsGroupMask is a bit mask describing which groups are listed in the report.
    pub fn ans_group_mask(&self) -> u8 {
        self.0[0] & 0b1111
    }

    /// NbTotalGroups is the number of multicast groups currently defined in the end-device.
    pub fn nb_total_groups(&self) -> u8 {
        (self.0[0] >> 4) & 0b111
    }

    /// Maximum possible length of the payload
    pub const fn max_len() -> usize {
        1 + MAX_GROUPS * Self::ITEM_LEN
    }

    /// Actual length of this specific payload
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        // status byte + number of groups * group length
        1 + Self::required_len(self.0[0])
    }

    pub fn item_iterator(&self) -> McGroupStatusItemIterator<'a> {
        McGroupStatusItemIterator { data: &self.0[1..], pos: 0 }
    }
}

pub struct McGroupStatusItemIterator<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for McGroupStatusItemIterator<'a> {
    type Item = McGroupStatusItem<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos + McGroupStatusItem::len() > self.data.len() {
            return None;
        }
        let item = McGroupStatusItem(&self.data[self.pos..self.pos + McGroupStatusItem::len()]);
        self.pos += McGroupStatusItem::len();
        Some(item)
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct McGroupStatusAnsCreator {
    pub(crate) data: [u8; McGroupStatusAnsPayload::max_len() + 1],
    items: usize,
}

impl McGroupStatusAnsCreator {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let mut data = [0; McGroupStatusAnsPayload::max_len() + 1];
        data[0] = McGroupStatusAnsPayload::cid();
        Self { data, items: 0 }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        // cid + status + items * item_len
        1 + 1 + self.items * McGroupStatusItem::len()
    }

    pub fn nb_total_groups(&mut self, nb_total_groups: u8) -> &mut Self {
        self.data[1] &= 0b1111;
        self.data[1] |= (nb_total_groups & 0b111) << 4;
        self
    }

    pub fn push<T: AsRef<[u8]>>(
        &mut self,
        group_id: u8,
        mc_addr: McAddr<T>,
    ) -> Result<&mut Self, Error> {
        // update bitmask in status byte
        let bm = 1 << group_id;
        self.data[1] |= bm;
        let offset = 2 + self.items * McGroupStatusItem::len();
        self.data[offset] = group_id;
        self.data[offset + 1..offset + McGroupStatusItem::len()].copy_from_slice(mc_addr.as_ref());
        self.items += 1;
        Ok(self)
    }

    pub fn build(&self) -> &[u8] {
        &self.data[..self.len()]
    }
}

impl McGroupStatusReqCreator {
    pub fn req_group_mask(&mut self, req_group_mask: u8) {
        self.data[1] &= 0b11110000;
        self.data[1] |= req_group_mask & 0b1111;
    }

    pub fn req_group(&mut self, req_group: u8) {
        // set just the bit
        let bm = 1 << (req_group & 0b11);
        self.data[1] |= bm;
    }
}

impl McGroupStatusReqPayload<'_> {
    pub fn req_group_mask(&self) -> u8 {
        self.0[0] & 0b1111
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::multicast::{
        parse_downlink_multicast_messages, parse_uplink_multicast_messages, DownlinkRemoteSetup,
        UplinkRemoteSetup,
    };
    #[test]
    fn roundtrip_one_group() {
        let mut creator = McGroupStatusAnsCreator::new();
        creator.nb_total_groups(2);
        let mc_addr = McAddr::new_from_raw(&[1, 2, 3, 4]);
        creator.push(0, mc_addr).unwrap();
        let payload = creator.build();
        assert_eq!(payload.len(), 7);
        let message = parse_uplink_multicast_messages(payload).next().unwrap();
        if let UplinkRemoteSetup::McGroupStatusAns(payload) = message {
            assert_eq!(payload.nb_total_groups(), 2);
            assert_eq!(payload.ans_group_mask(), 1);
            let mut iter = payload.item_iterator();
            let item = iter.next().unwrap();
            assert_eq!(item.mc_group_id(), 0);
            assert_eq!(item.mc_addr().as_ref(), &[1, 2, 3, 4]);
            assert!(iter.next().is_none());
        } else {
            panic!("Expected McGroupStatusAns");
        }
    }

    #[test]
    fn roundtrip_all_groups() {
        let mut creator = McGroupStatusAnsCreator::new();
        creator.nb_total_groups(4);
        let mc_addr = McAddr::new_from_raw(&[1, 1, 1, 1]);
        creator.push(0, mc_addr).unwrap();
        let mc_addr = McAddr::new_from_raw(&[2, 2, 2, 2]);
        creator.push(1, mc_addr).unwrap();
        let mc_addr = McAddr::new_from_raw(&[3, 3, 3, 3]);
        creator.push(2, mc_addr).unwrap();
        let mc_addr = McAddr::new_from_raw(&[4, 4, 4, 4]);
        creator.push(3, mc_addr).unwrap();
        let payload = creator.build();
        //assert_eq!(payload.len(), 22);
        let message = parse_uplink_multicast_messages(payload).next().unwrap();
        if let UplinkRemoteSetup::McGroupStatusAns(payload) = message {
            assert_eq!(payload.nb_total_groups(), 4);
            assert_eq!(payload.ans_group_mask(), 0b1111);
            let mut iter = payload.item_iterator();
            let item = iter.next().unwrap();
            assert_eq!(item.mc_group_id(), 0);
            assert_eq!(item.mc_addr().as_ref(), &[1, 1, 1, 1]);
            let item = iter.next().unwrap();
            assert_eq!(item.mc_group_id(), 1);
            assert_eq!(item.mc_addr().as_ref(), &[2, 2, 2, 2]);
            let item = iter.next().unwrap();
            assert_eq!(item.mc_group_id(), 2);
            assert_eq!(item.mc_addr().as_ref(), &[3, 3, 3, 3]);
            let item = iter.next().unwrap();
            assert_eq!(item.mc_group_id(), 3);
            assert_eq!(item.mc_addr().as_ref(), &[4, 4, 4, 4]);
            assert!(iter.next().is_none());
        } else {
            panic!("Expected McGroupStatusAns");
        }
    }

    #[test]
    fn roundtrip_request() {
        let mut creator = McGroupStatusReqCreator::new();
        creator.req_group(0);
        creator.req_group(2);

        let payload = creator.build();
        let message = parse_downlink_multicast_messages(payload).next().unwrap();
        if let DownlinkRemoteSetup::McGroupStatusReq(payload) = message {
            assert_eq!(payload.req_group_mask(), 0b101);
        } else {
            panic!("Expected McGroupStatusReq");
        }
    }
}
