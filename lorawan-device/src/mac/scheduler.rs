/// The uplink scheduler.
///
/// Owns the pending package answers in order of priority:
/// - certification (FPort 224)
/// - multicast setup answer (FPort 200).
///
/// A pending answer is sent on the next uplink, replacing the requested
/// application data.
pub(crate) struct Scheduler {
    /// The staged certification answer.
    #[cfg(feature = "certification")]
    certification: Option<heapless::Vec<u8, 256>>,
    /// The staged multicast setup answers.
    #[cfg(feature = "multicast")]
    multicast_setup: Option<heapless::Vec<u8, 256>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "certification")]
            certification: None,
            #[cfg(feature = "multicast")]
            multicast_setup: None,
        }
    }

    /// Stage a certification answer, replacing a previously staged one.
    #[cfg(feature = "certification")]
    pub(crate) fn stage_certification(&mut self, answer: heapless::Vec<u8, 256>) {
        self.certification = Some(answer)
    }

    /// Append a multicast setup answer to the staged answers.
    #[cfg(feature = "multicast")]
    pub(crate) fn stage_multicast_setup(&mut self, answer: &heapless::Vec<u8, 256>) {
        match &mut self.multicast_setup {
            Some(staged) => staged.extend_from_slice(answer).unwrap(),
            None => self.multicast_setup = Some(heapless::Vec::from_slice(answer).unwrap()),
        }
    }

    /// Take the staged certification answer, if any.
    #[cfg(feature = "certification")]
    pub(crate) fn take_due_certification(&mut self) -> Option<heapless::Vec<u8, 256>> {
        self.certification.take()
    }

    /// Take the staged multicast setup answers, if any.
    #[cfg(feature = "multicast")]
    pub(crate) fn take_due_multicast_setup(&mut self) -> Option<heapless::Vec<u8, 256>> {
        self.multicast_setup.take()
    }

    /// Drop the pending package answers; the session is being reset.
    #[cfg(feature = "certification")]
    pub(crate) fn reset(&mut self) {
        *self = Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::Scheduler;
    use heapless::Vec;

    fn answer(bytes: &[u8]) -> Vec<u8, 256> {
        Vec::from_slice(bytes).unwrap()
    }

    #[cfg(feature = "certification")]
    #[test]
    fn certification_answer_is_staged_and_taken_once() {
        let mut scheduler = Scheduler::new();
        assert!(scheduler.take_due_certification().is_none());
        scheduler.stage_certification(answer(&[1, 2, 3]));
        assert_eq!(scheduler.take_due_certification(), Some(answer(&[1, 2, 3])));
        assert!(scheduler.take_due_certification().is_none());
    }

    #[cfg(feature = "certification")]
    #[test]
    fn certification_answer_replaces_the_previously_staged_one() {
        let mut scheduler = Scheduler::new();
        scheduler.stage_certification(answer(&[1]));
        scheduler.stage_certification(answer(&[2, 3]));
        assert_eq!(scheduler.take_due_certification(), Some(answer(&[2, 3])));
    }

    #[cfg(feature = "multicast")]
    #[test]
    fn multicast_setup_answers_accumulate() {
        let mut scheduler = Scheduler::new();
        scheduler.stage_multicast_setup(&answer(&[1]));
        scheduler.stage_multicast_setup(&answer(&[2, 3]));
        assert_eq!(scheduler.take_due_multicast_setup(), Some(answer(&[1, 2, 3])));
        assert!(scheduler.take_due_multicast_setup().is_none());
    }

    #[cfg(feature = "certification")]
    #[test]
    fn reset_drops_the_pending_answers() {
        let mut scheduler = Scheduler::new();
        #[cfg(feature = "certification")]
        scheduler.stage_certification(answer(&[1]));
        #[cfg(feature = "multicast")]
        scheduler.stage_multicast_setup(&answer(&[2]));
        scheduler.reset();
        #[cfg(feature = "certification")]
        assert!(scheduler.take_due_certification().is_none());
        #[cfg(feature = "multicast")]
        assert!(scheduler.take_due_multicast_setup().is_none());
    }
}
