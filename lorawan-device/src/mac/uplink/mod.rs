use lorawan::maccommands::SerializableMacCommand;
use lorawan::packet_length::phy::mac::fhdr::FOPTS_MAX_LEN;

#[cfg(feature = "serde")]
mod serde;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Uplink {
    pending: heapless::Vec<u8, FOPTS_MAX_LEN>,
    confirmed: bool,
}

impl Uplink {
    pub fn set_downlink_confirmation(&mut self) {
        self.confirmed = true;
    }
    pub fn clear_downlink_confirmation(&mut self) {
        self.confirmed = false;
    }
    pub fn confirms_downlink(&self) -> bool {
        self.confirmed
    }
    pub fn add_mac_command<M: SerializableMacCommand>(&mut self, cmd: M) {
        let _ = self.pending.push(cmd.cid());
        self.pending.extend_from_slice(cmd.payload_bytes()).unwrap();
    }

    pub fn clear_mac_commands(&mut self) {
        self.pending.clear();
    }
    pub fn mac_commands(&self) -> &[u8] {
        &self.pending
    }
}

#[cfg(feature = "defmt-03")]
impl defmt::Format for Uplink {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(
            f,
            "Uplink {{ pending.len(): {}, confirmed: {} }}",
            self.pending.len(),
            self.confirmed
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use lorawan::maccommands::{parse_uplink_mac_commands, LinkADRAnsCreator, UplinkMacCommand};
    #[test]
    fn two_link_adr_ans() {
        let mut uplink = Uplink::default();
        uplink.add_mac_command(LinkADRAnsCreator::new());
        uplink.add_mac_command(LinkADRAnsCreator::new());
        let mut mac_commands = parse_uplink_mac_commands(uplink.mac_commands());
        assert!(matches!(mac_commands.next().unwrap(), UplinkMacCommand::LinkADRAns(_)));
        assert!(matches!(mac_commands.next().unwrap(), UplinkMacCommand::LinkADRAns(_)));
        assert!(mac_commands.next().is_none());
    }
}
