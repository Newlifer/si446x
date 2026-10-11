#![forbid(unsafe_code)]

use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use embedded_hal::spi::Operation;
use si446x::{CtsConfig, Error};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MockError;

impl embedded_hal::spi::Error for MockError {
    fn kind(&self) -> embedded_hal::spi::ErrorKind {
        embedded_hal::spi::ErrorKind::Other
    }
}

impl embedded_hal::digital::Error for MockError {
    fn kind(&self) -> embedded_hal::digital::ErrorKind {
        embedded_hal::digital::ErrorKind::Other
    }
}

#[derive(Debug)]
enum Wire {
    Write(Vec<u8>),
    Read(Vec<u8>),
}

#[derive(Debug)]
enum Step {
    Spi(Vec<Wire>, Result<(), MockError>),
    Pin(bool, Result<(), MockError>),
    Delay(u32),
}

fn cts(value: u8) -> Step {
    Step::Spi(
        vec![Wire::Write(vec![0x44]), Wire::Read(vec![value])],
        Ok(()),
    )
}

fn request() -> Step {
    Step::Spi(vec![Wire::Write(vec![0x01])], Ok(()))
}

fn response(status: u8) -> Step {
    Step::Spi(
        vec![
            Wire::Write(vec![0x44]),
            Wire::Read(vec![status]),
            Wire::Read(vec![0x44, 0x60]),
        ],
        Ok(()),
    )
}

fn failing(mut step: Step) -> Step {
    match &mut step {
        Step::Spi(_, result) | Step::Pin(_, result) => *result = Err(MockError),
        Step::Delay(_) => unreachable!("a delay cannot return an error"),
    }
    step
}

// One shared script verifies ordering across SPI, SDN and delay calls. Each
// Spi step must correspond to exactly one SpiDevice transaction (one CS scope).
#[derive(Clone)]
struct Script(Rc<RefCell<VecDeque<Step>>>);

impl Script {
    fn new(steps: Vec<Step>) -> Self {
        Self(Rc::new(RefCell::new(steps.into())))
    }

    fn next(&self) -> Option<Step> {
        let step = self.0.borrow_mut().pop_front();
        assert!(step.is_some(), "unexpected hardware operation");
        step
    }

    fn done(&self) {
        assert!(self.0.borrow().is_empty(), "unconsumed steps: {:?}", self.0);
    }

    fn spi(&self, operations: &mut [Operation<'_, u8>]) -> Result<(), MockError> {
        let step = self.next();
        assert!(
            matches!(step, Some(Step::Spi(..))),
            "expected SPI: {step:?}"
        );
        let Some(Step::Spi(wires, result)) = step else {
            return Err(MockError);
        };
        assert_eq!(operations.len(), wires.len());
        for (operation, wire) in operations.iter_mut().zip(wires) {
            match (operation, wire) {
                (Operation::Write(actual), Wire::Write(expected)) => {
                    assert_eq!(*actual, expected);
                }
                (Operation::Read(actual), Wire::Read(data)) => {
                    assert_eq!(actual.len(), data.len());
                    if result.is_ok() {
                        actual.copy_from_slice(&data);
                    }
                }
                (actual, expected) => {
                    unreachable!("operation mismatch: {actual:?} vs {expected:?}");
                }
            }
        }
        result
    }

    fn pin(&self, high: bool) -> Result<(), MockError> {
        let step = self.next();
        assert!(
            matches!(step, Some(Step::Pin(..))),
            "expected pin: {step:?}"
        );
        let Some(Step::Pin(expected, result)) = step else {
            return Err(MockError);
        };
        assert_eq!(high, expected);
        result
    }

    fn delay(&self, ns: u32) {
        let step = self.next();
        assert!(
            matches!(step, Some(Step::Delay(..))),
            "expected delay: {step:?}"
        );
        if let Some(Step::Delay(expected)) = step {
            assert_eq!(ns, expected);
        }
    }
}

struct Spi(Script);
struct Pin(Script);
struct Delay(Script);

impl embedded_hal::spi::ErrorType for Spi {
    type Error = MockError;
}

impl embedded_hal::spi::SpiDevice for Spi {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), MockError> {
        self.0.spi(operations)
    }
}

#[cfg(feature = "async")]
impl embedded_hal_async::spi::SpiDevice for Spi {
    async fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), MockError> {
        self.0.spi(operations)
    }
}

impl embedded_hal::digital::ErrorType for Pin {
    type Error = MockError;
}

impl embedded_hal::digital::OutputPin for Pin {
    fn set_low(&mut self) -> Result<(), MockError> {
        self.0.pin(false)
    }

    fn set_high(&mut self) -> Result<(), MockError> {
        self.0.pin(true)
    }
}

impl embedded_hal::delay::DelayNs for Delay {
    fn delay_ns(&mut self, ns: u32) {
        self.0.delay(ns);
    }
}

#[cfg(feature = "async")]
impl embedded_hal_async::delay::DelayNs for Delay {
    async fn delay_ns(&mut self, ns: u32) {
        self.0.delay(ns);
    }
}

// Run precisely the same observable-behavior scenarios against both APIs.
macro_rules! transport_tests {
    () => {
        fn setup(steps: Vec<Step>) -> (Si446x<Spi, Pin>, Delay, Script) {
            let script = Script::new(steps);
            let mut radio = Si446x::new(Spi(script.clone()), Pin(script.clone()));
            radio.set_cts_config(CtsConfig {
                max_attempts: 3,
                poll_interval_ns: 17,
            });
            (radio, Delay(script.clone()), script)
        }

        #[test]
        fn ready_command_reads_response_in_one_cs_scope() {
            let (mut radio, mut delay, script) =
                setup(vec![cts(0xff), request(), cts(0xff), response(0xff)]);
            let mut bytes = [0; 2];
            assert_eq!(run!(radio.command(&[0x01], &mut bytes, &mut delay)), Ok(()));
            assert_eq!(bytes, [0x44, 0x60]);
            script.done();
        }

        #[test]
        fn no_response_still_waits_for_completion() {
            let (mut radio, mut delay, script) = setup(vec![
                cts(0xff),
                request(),
                cts(0),
                Step::Delay(17),
                cts(0xff),
            ]);
            assert_eq!(run!(radio.command(&[0x01], &mut [], &mut delay)), Ok(()));
            script.done();
        }

        #[test]
        fn waits_before_writing_and_accepts_last_attempt() {
            let (mut radio, mut delay, script) = setup(vec![
                cts(0),
                Step::Delay(17),
                cts(0xfe),
                Step::Delay(17),
                cts(0xff),
                request(),
                cts(0),
                Step::Delay(17),
                cts(0),
                Step::Delay(17),
                cts(0xff),
            ]);
            assert_eq!(run!(radio.command(&[0x01], &mut [], &mut delay)), Ok(()));
            script.done();
        }

        #[test]
        fn timeout_before_write_does_not_send_request_or_delay_after_last_poll() {
            let (mut radio, mut delay, script) = setup(vec![
                cts(0),
                Step::Delay(17),
                cts(0),
                Step::Delay(17),
                cts(0),
            ]);
            assert_eq!(
                run!(radio.command(&[0x01], &mut [], &mut delay)),
                Err(Error::CtsTimeout)
            );
            script.done();
        }

        #[test]
        fn timeout_after_write_preserves_response_and_next_command_waits_again() {
            let (mut radio, mut delay, script) = setup(vec![
                cts(0xff),
                request(),
                cts(0),
                Step::Delay(17),
                cts(0),
                Step::Delay(17),
                cts(0),
                cts(0),
                Step::Delay(17),
                cts(0xff),
                request(),
                cts(0xff),
                response(0xff),
            ]);
            let mut bytes = [0xaa; 2];
            assert_eq!(
                run!(radio.command(&[0x01], &mut bytes, &mut delay)),
                Err(Error::CtsTimeout)
            );
            assert_eq!(bytes, [0xaa; 2]);
            assert_eq!(run!(radio.command(&[0x01], &mut bytes, &mut delay)), Ok(()));
            assert_eq!(bytes, [0x44, 0x60]);
            script.done();
        }

        #[test]
        fn empty_request_causes_no_io() {
            let (mut radio, mut delay, script) = setup(vec![]);
            assert_eq!(
                run!(radio.command(&[], &mut [], &mut delay)),
                Err(Error::InvalidCommand)
            );
            script.done();
        }

        #[test]
        fn zero_attempts_rejects_command_and_wait_without_io() {
            let (mut radio, mut delay, script) = setup(vec![]);
            radio.set_cts_config(CtsConfig {
                max_attempts: 0,
                poll_interval_ns: 17,
            });
            assert_eq!(
                run!(radio.command(&[0x01], &mut [], &mut delay)),
                Err(Error::InvalidConfig)
            );
            assert_eq!(run!(radio.wait_cts(&mut delay)), Err(Error::InvalidConfig));
            script.done();
        }

        #[test]
        fn one_attempt_never_delays() {
            let (mut radio, mut delay, script) = setup(vec![cts(0xff), cts(0)]);
            radio.set_cts_config(CtsConfig {
                max_attempts: 1,
                poll_interval_ns: 17,
            });
            assert_eq!(run!(radio.wait_cts(&mut delay)), Ok(()));
            assert_eq!(run!(radio.wait_cts(&mut delay)), Err(Error::CtsTimeout));
            script.done();
        }

        #[test]
        fn zero_poll_interval_is_valid() {
            let (mut radio, mut delay, script) = setup(vec![cts(0), Step::Delay(0), cts(0xff)]);
            radio.set_cts_config(CtsConfig {
                max_attempts: 2,
                poll_interval_ns: 0,
            });
            assert_eq!(run!(radio.wait_cts(&mut delay)), Ok(()));
            script.done();
        }

        #[test]
        fn poll_cts_only_accepts_ff_and_does_not_delay() {
            let (mut radio, _, script) = setup(vec![cts(0), cts(0xfe), cts(0xff)]);
            assert_eq!(run!(radio.poll_cts()), Ok(false));
            assert_eq!(run!(radio.poll_cts()), Ok(false));
            assert_eq!(run!(radio.poll_cts()), Ok(true));
            script.done();
        }

        #[test]
        fn spi_errors_stop_at_each_command_stage() {
            for steps in [
                vec![failing(cts(0xff))],
                vec![cts(0xff), failing(request())],
                vec![cts(0xff), request(), failing(cts(0xff))],
                vec![cts(0xff), request(), cts(0xff), failing(response(0xff))],
            ] {
                let (mut radio, mut delay, script) = setup(steps);
                assert_eq!(
                    run!(radio.command(&[0x01], &mut [0; 2], &mut delay)),
                    Err(Error::Spi(MockError))
                );
                script.done();
            }
        }

        #[test]
        fn next_call_rechecks_cts_after_spi_error() {
            let (mut radio, mut delay, script) = setup(vec![
                cts(0xff),
                request(),
                failing(cts(0xff)),
                cts(0),
                Step::Delay(17),
                cts(0xff),
                request(),
                cts(0xff),
            ]);
            assert_eq!(
                run!(radio.command(&[0x01], &mut [], &mut delay)),
                Err(Error::Spi(MockError))
            );
            assert_eq!(run!(radio.command(&[0x01], &mut [], &mut delay)), Ok(()));
            script.done();
        }

        #[test]
        fn unexpected_response_cts_is_rejected() {
            let (mut radio, mut delay, script) =
                setup(vec![cts(0xff), request(), cts(0xff), response(0)]);
            assert_eq!(
                run!(radio.command(&[0x01], &mut [0; 2], &mut delay)),
                Err(Error::InvalidResponse)
            );
            script.done();
        }

        #[test]
        fn reset_pulses_sdn_with_required_delays_then_command_checks_cts() {
            let (mut radio, mut delay, script) = setup(vec![
                Step::Pin(true, Ok(())),
                Step::Delay(20_000),
                Step::Pin(false, Ok(())),
                Step::Delay(20_000_000),
                cts(0),
                Step::Delay(17),
                cts(0xff),
                request(),
                cts(0xff),
            ]);
            assert_eq!(run!(radio.reset(&mut delay)), Ok(()));
            assert_eq!(run!(radio.command(&[0x01], &mut [], &mut delay)), Ok(()));
            script.done();
        }

        #[test]
        fn sdn_errors_stop_reset_immediately() {
            for steps in [
                vec![Step::Pin(true, Err(MockError))],
                vec![
                    Step::Pin(true, Ok(())),
                    Step::Delay(20_000),
                    Step::Pin(false, Err(MockError)),
                ],
            ] {
                let (mut radio, mut delay, script) = setup(steps);
                assert_eq!(run!(radio.reset(&mut delay)), Err(Error::Sdn(MockError)));
                script.done();
            }
        }

        #[test]
        fn release_returns_owned_devices_and_configuration_round_trips() {
            let script = Script::new(vec![]);
            let mut radio = Si446x::new(Spi(script.clone()), Pin(script.clone()));
            assert_eq!(radio.cts_config(), CtsConfig::default());
            let config = CtsConfig {
                max_attempts: 7,
                poll_interval_ns: 123,
            };
            radio.set_cts_config(config);
            assert_eq!(radio.cts_config(), config);
            let (spi, pin) = radio.release();
            assert!(Rc::ptr_eq(&spi.0.0, &script.0));
            assert!(Rc::ptr_eq(&pin.0.0, &script.0));
            script.done();
        }
    };
}

mod blocking {
    use super::*;
    use si446x::blocking::Si446x;

    macro_rules! run {
        ($future:expr) => {
            $future
        };
    }

    transport_tests!();
}

#[cfg(feature = "async")]
mod asynch {
    use super::*;
    use si446x::asynch::Si446x;
    use std::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };

    // Normal mocks complete synchronously. Unexpected suspension fails rather
    // than hanging the tests; cancellation tests explicitly expect Pending.
    fn run_ready<F: Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let result = pin!(future).poll(&mut context);
        assert!(result.is_ready(), "mock unexpectedly suspended");
        match result {
            Poll::Ready(value) => value,
            Poll::Pending => unreachable!("checked readiness above"),
        }
    }

    macro_rules! run {
        ($future:expr) => {
            run_ready($future)
        };
    }

    transport_tests!();

    struct SuspendedDelay(Script);

    impl embedded_hal_async::delay::DelayNs for SuspendedDelay {
        async fn delay_ns(&mut self, ns: u32) {
            self.0.delay(ns);
            std::future::pending::<()>().await;
        }
    }

    fn cancel_pending(future: impl Future) {
        let mut context = Context::from_waker(Waker::noop());
        assert!(pin!(future).poll(&mut context).is_pending());
        // Drop the future while suspended to release its driver borrow.
    }

    #[test]
    fn cancellation_before_write_does_not_send_and_next_call_rechecks_cts() {
        let (mut radio, mut delay, script) = setup(vec![
            cts(0),
            Step::Delay(17),
            cts(0),
            Step::Delay(17),
            cts(0xff),
            request(),
            cts(0xff),
        ]);
        let mut suspended = SuspendedDelay(script.clone());
        cancel_pending(radio.command(&[0x01], &mut [], &mut suspended));
        assert_eq!(run!(radio.command(&[0x01], &mut [], &mut delay)), Ok(()));
        script.done();
    }

    #[test]
    fn cancellation_after_write_abandons_response_and_next_call_rechecks_cts() {
        let (mut radio, mut delay, script) = setup(vec![
            cts(0xff),
            request(),
            cts(0),
            Step::Delay(17),
            cts(0),
            Step::Delay(17),
            cts(0xff),
            request(),
            cts(0xff),
            response(0xff),
        ]);
        let mut suspended = SuspendedDelay(script.clone());
        let mut abandoned = [0xaa; 2];
        cancel_pending(radio.command(&[0x01], &mut abandoned, &mut suspended));
        assert_eq!(abandoned, [0xaa; 2]);
        let mut bytes = [0; 2];
        assert_eq!(run!(radio.command(&[0x01], &mut bytes, &mut delay)), Ok(()));
        assert_eq!(bytes, [0x44, 0x60]);
        script.done();
    }

    #[test]
    fn cancelled_reset_can_be_restarted_from_sdn_high() {
        let (mut radio, mut delay, script) = setup(vec![
            Step::Pin(true, Ok(())),
            Step::Delay(20_000),
            Step::Pin(true, Ok(())),
            Step::Delay(20_000),
            Step::Pin(false, Ok(())),
            Step::Delay(20_000_000),
        ]);
        let mut suspended = SuspendedDelay(script.clone());
        cancel_pending(radio.reset(&mut suspended));
        assert_eq!(run!(radio.reset(&mut delay)), Ok(()));
        script.done();
    }
}
