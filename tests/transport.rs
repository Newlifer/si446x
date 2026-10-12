#![forbid(unsafe_code)]

use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use embedded_hal::spi::Operation;
use si446x::{
    ConfigurationError, CtsConfig, DeviceState, DriveStrength, Error, FuncInfo, GpioPinConfig,
    GpioPinStatus, PartInfo, PinConfig, PowerUp, State,
};

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

fn exchange(request: &[u8], data: &[u8]) -> Vec<Step> {
    let mut steps = vec![
        cts(0xff),
        Step::Spi(vec![Wire::Write(request.to_vec())], Ok(())),
        cts(0xff),
    ];
    if !data.is_empty() {
        steps.push(Step::Spi(
            vec![
                Wire::Write(vec![0x44]),
                Wire::Read(vec![0xff]),
                Wire::Read(data.to_vec()),
            ],
            Ok(()),
        ));
    }
    steps
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
        fn properties_write_and_read_the_requested_range() {
            let mut steps = exchange(&[0x11, 0x20, 2, 0x10, 0xaa, 0xbb], &[]);
            steps.extend(exchange(&[0x12, 0x20, 2, 0x10], &[0xaa, 0xbb]));
            let (mut radio, mut delay, script) = setup(steps);
            assert_eq!(
                run!(radio.set_property(0x20, 0x10, &[0xaa, 0xbb], &mut delay)),
                Ok(())
            );
            let mut values = [0; 2];
            assert_eq!(
                run!(radio.get_property(0x20, 0x10, &mut values, &mut delay)),
                Ok(())
            );
            assert_eq!(values, [0xaa, 0xbb]);
            script.done();
        }

        #[test]
        fn properties_accept_maximum_counts_at_group_end() {
            let mut request = vec![0x11, 0x20, 12, 244];
            request.extend([0xab; 12]);
            let mut steps = exchange(&request, &[]);
            steps.extend(exchange(&[0x12, 0x20, 16, 240], &[0xcd; 16]));
            let (mut radio, mut delay, script) = setup(steps);
            assert_eq!(
                run!(radio.set_property(0x20, 244, &[0xab; 12], &mut delay)),
                Ok(())
            );
            let mut values = [0; 16];
            assert_eq!(
                run!(radio.get_property(0x20, 240, &mut values, &mut delay)),
                Ok(())
            );
            assert_eq!(values, [0xcd; 16]);
            script.done();
        }

        #[test]
        fn invalid_property_ranges_cause_no_io() {
            let (mut radio, mut delay, script) = setup(vec![]);
            for (start, count) in [(0, 0), (0, 13), (255, 2)] {
                assert_eq!(
                    run!(radio.set_property(0x20, start, &vec![0; count], &mut delay)),
                    Err(Error::InvalidConfig)
                );
            }
            for (start, count) in [(0, 0), (0, 17), (255, 2), (0, 257)] {
                assert_eq!(
                    run!(radio.get_property(0x20, start, &mut vec![0; count], &mut delay)),
                    Err(Error::InvalidConfig)
                );
            }
            script.done();
        }

        #[test]
        fn configuration_with_invalid_cts_settings_performs_no_spi_io() {
            let (mut radio, mut delay, script) = setup(vec![]);
            radio.set_cts_config(CtsConfig {
                max_attempts: 0,
                poll_interval_ns: 0,
            });
            assert_eq!(
                run!(radio.load_configuration(&[2, 0x34, 3, 0], &mut delay)),
                Err(Error::InvalidConfig)
            );
            assert_eq!(
                run!(radio.change_state(State::Sleep, &mut delay)),
                Err(Error::InvalidConfig)
            );
            script.done();
        }

        #[test]
        fn configuration_is_fully_validated_before_any_spi_operation() {
            let (mut radio, mut delay, script) = setup(vec![]);
            assert_eq!(
                run!(radio.load_configuration(&[5, 0x11, 0x20, 1, 0, 3, 2, 0x34], &mut delay)),
                Err(Error::InvalidConfiguration(ConfigurationError::Truncated {
                    offset: 6
                }))
            );
            assert_eq!(
                run!(radio.load_configuration(&[1, 0, 2, 0x34, 4, 0], &mut delay)),
                Err(Error::InvalidConfiguration(
                    ConfigurationError::InvalidCommand { offset: 2 }
                ))
            );
            assert_eq!(run!(radio.load_configuration(&[0], &mut delay)), Ok(()));
            script.done();
        }

        #[test]
        fn configuration_executes_records_in_order_and_discards_response_data() {
            let mut steps = exchange(&[0x11, 0x20, 1, 0, 3], &[]);
            steps.extend(exchange(&[0x13, 4, 8, 0, 0, 39, 11, 0], &[]));
            steps.extend(exchange(&[0x34, 3], &[]));
            let (mut radio, mut delay, script) = setup(steps);
            let bytes = [
                5, 0x11, 0x20, 1, 0, 3, 8, 0x13, 4, 8, 0, 0, 39, 11, 0, 2, 0x34, 3, 0,
            ];
            assert_eq!(run!(radio.load_configuration(&bytes, &mut delay)), Ok(()));
            script.done();
        }

        #[test]
        fn configuration_stops_on_spi_error_and_does_not_send_later_records() {
            let mut steps = exchange(&[0x11, 0x20, 1, 0, 3], &[]);
            steps.extend([
                cts(0xff),
                failing(Step::Spi(vec![Wire::Write(vec![0x34, 3])], Ok(()))),
            ]);
            let (mut radio, mut delay, script) = setup(steps);
            assert_eq!(
                run!(radio.load_configuration(
                    &[5, 0x11, 0x20, 1, 0, 3, 2, 0x34, 3, 1, 0, 0],
                    &mut delay
                )),
                Err(Error::Spi(MockError))
            );
            script.done();
        }

        #[test]
        fn configuration_stops_on_cts_timeout() {
            let (mut radio, mut delay, script) = setup(vec![
                cts(0xff),
                Step::Spi(vec![Wire::Write(vec![0x34, 3])], Ok(())),
                cts(0),
                Step::Delay(17),
                cts(0),
                Step::Delay(17),
                cts(0),
            ]);
            assert_eq!(
                run!(radio.load_configuration(&[2, 0x34, 3, 1, 0, 0], &mut delay)),
                Err(Error::CtsTimeout)
            );
            script.done();
        }

        #[test]
        fn gpio_configuration_reads_modes_levels_and_drive_strength() {
            let reply = [0x84, 8, 0xc2, 3, 0xa7, 11, 0xa0];
            let (mut radio, mut delay, script) =
                setup(exchange(&[0x13, 4, 0x48, 2, 3, 39, 11, 0x20], &reply));
            let config = GpioPinConfig {
                gpio: [
                    PinConfig::INPUT,
                    PinConfig {
                        mode: 8,
                        pull_up: true,
                    },
                    PinConfig::LOW,
                    PinConfig::HIGH,
                ],
                nirq: PinConfig::NIRQ,
                sdo: PinConfig::SDO,
                drive_strength: DriveStrength::MediumHigh,
            };
            assert_eq!(
                run!(radio.gpio_pin_cfg(config, &mut delay)),
                Ok(GpioPinStatus::from_bytes(reply))
            );
            script.done();
        }

        #[test]
        fn invalid_gpio_modes_cause_no_io() {
            let (mut radio, mut delay, script) = setup(vec![]);
            for config in [
                GpioPinConfig {
                    gpio: [PinConfig {
                        mode: 30,
                        pull_up: false,
                    }; 4],
                    ..GpioPinConfig::default()
                },
                GpioPinConfig {
                    nirq: PinConfig {
                        mode: 5,
                        pull_up: false,
                    },
                    ..GpioPinConfig::default()
                },
                GpioPinConfig {
                    sdo: PinConfig::NIRQ,
                    ..GpioPinConfig::default()
                },
            ] {
                assert_eq!(
                    run!(radio.gpio_pin_cfg(config, &mut delay)),
                    Err(Error::InvalidConfig)
                );
            }
            script.done();
        }

        #[test]
        fn state_change_and_query_decode_the_current_state_and_channel() {
            let mut steps = exchange(&[0x34, 3], &[]);
            steps.extend(exchange(&[0x33], &[0xa4, 17]));
            let (mut radio, mut delay, script) = setup(steps);
            assert_eq!(run!(radio.change_state(State::Ready, &mut delay)), Ok(()));
            assert_eq!(
                run!(radio.request_device_state(&mut delay)),
                Ok(DeviceState {
                    main_state: 4,
                    channel: 17
                })
            );
            script.done();
        }

        #[test]
        fn sleep_is_not_followed_by_a_waking_spi_poll() {
            let (mut radio, mut delay, script) = setup(vec![
                cts(0),
                Step::Delay(17),
                cts(0xff),
                Step::Spi(vec![Wire::Write(vec![0x34, 1])], Ok(())),
            ]);
            assert_eq!(run!(radio.change_state(State::Sleep, &mut delay)), Ok(()));
            script.done();
        }

        #[test]
        fn final_sleep_in_configuration_is_not_followed_by_spi_poll() {
            let mut steps = exchange(&[0x34, 3], &[]);
            steps.extend([
                cts(0xff),
                Step::Spi(vec![Wire::Write(vec![0x34, 1])], Ok(())),
            ]);
            let (mut radio, mut delay, script) = setup(steps);
            assert_eq!(
                run!(radio.load_configuration(&[2, 0x34, 3, 2, 0x34, 1, 0], &mut delay)),
                Ok(())
            );
            script.done();
        }

        #[test]
        fn new_commands_propagate_transport_errors() {
            let (mut radio, mut delay, script) = setup(vec![failing(cts(0xff))]);
            assert_eq!(
                run!(radio.set_property(0, 0, &[1], &mut delay)),
                Err(Error::Spi(MockError))
            );
            script.done();
            let (mut radio, mut delay, script) = setup(vec![failing(cts(0xff))]);
            assert_eq!(
                run!(radio.get_property(0, 0, &mut [0], &mut delay)),
                Err(Error::Spi(MockError))
            );
            script.done();
            let (mut radio, mut delay, script) = setup(vec![failing(cts(0xff))]);
            assert_eq!(
                run!(radio.gpio_pin_cfg(GpioPinConfig::default(), &mut delay)),
                Err(Error::Spi(MockError))
            );
            script.done();
            let (mut radio, mut delay, script) = setup(vec![failing(cts(0xff))]);
            assert_eq!(
                run!(radio.change_state(State::Sleep, &mut delay)),
                Err(Error::Spi(MockError))
            );
            script.done();
            let (mut radio, mut delay, script) = setup(vec![failing(cts(0xff))]);
            assert_eq!(
                run!(radio.request_device_state(&mut delay)),
                Err(Error::Spi(MockError))
            );
            script.done();
        }

        #[test]
        fn typed_power_up_waits_for_completion_without_reading_data() {
            let (mut radio, mut delay, script) = setup(vec![
                cts(0xff),
                Step::Spi(
                    vec![Wire::Write(vec![0x02, 1, 0, 0x01, 0xc9, 0xc3, 0x80])],
                    Ok(()),
                ),
                cts(0),
                Step::Delay(17),
                cts(0xff),
            ]);
            assert_eq!(run!(radio.power_up(PowerUp::default(), &mut delay)), Ok(()));
            script.done();
        }

        #[test]
        fn typed_power_up_rejects_invalid_frequency_without_io() {
            let (mut radio, mut delay, script) = setup(vec![]);
            for frequency in [0, 24_999_999, 32_000_001, u32::MAX] {
                let config = PowerUp {
                    xo_frequency_hz: frequency,
                    ..PowerUp::default()
                };
                assert_eq!(
                    run!(radio.power_up(config, &mut delay)),
                    Err(Error::InvalidConfig)
                );
            }
            script.done();
        }

        #[test]
        fn typed_part_info_reads_exactly_eight_bytes_and_decodes_them() {
            let (mut radio, mut delay, script) = setup(vec![
                cts(0xff),
                request(),
                cts(0xff),
                Step::Spi(
                    vec![
                        Wire::Write(vec![0x44]),
                        Wire::Read(vec![0xff]),
                        Wire::Read(vec![0x11, 0x44, 0x60, 0x22, 0x12, 0x34, 0x33, 0x06]),
                    ],
                    Ok(()),
                ),
            ]);
            assert_eq!(
                run!(radio.part_info(&mut delay)),
                Ok(PartInfo {
                    chip_revision: 0x11,
                    part: 0x4460,
                    part_build: 0x22,
                    id: 0x1234,
                    customer: 0x33,
                    rom_id: 0x06,
                })
            );
            script.done();
        }

        #[test]
        fn typed_func_info_reads_exactly_six_bytes_and_decodes_them() {
            let (mut radio, mut delay, script) = setup(vec![
                cts(0xff),
                Step::Spi(vec![Wire::Write(vec![0x10])], Ok(())),
                cts(0xff),
                Step::Spi(
                    vec![
                        Wire::Write(vec![0x44]),
                        Wire::Read(vec![0xff]),
                        Wire::Read(vec![3, 4, 5, 0xab, 0xcd, 1]),
                    ],
                    Ok(()),
                ),
            ]);
            assert_eq!(
                run!(radio.func_info(&mut delay)),
                Ok(FuncInfo {
                    revision_external: 3,
                    revision_branch: 4,
                    revision_internal: 5,
                    patch: 0xabcd,
                    function: 1,
                })
            );
            script.done();
        }

        #[test]
        fn typed_methods_propagate_spi_errors() {
            let (mut radio, mut delay, script) = setup(vec![failing(cts(0xff))]);
            assert_eq!(
                run!(radio.power_up(PowerUp::default(), &mut delay)),
                Err(Error::Spi(MockError))
            );
            script.done();
            let (mut radio, mut delay, script) = setup(vec![cts(0xff), failing(request())]);
            assert_eq!(
                run!(radio.part_info(&mut delay)),
                Err(Error::Spi(MockError))
            );
            script.done();
            let (mut radio, mut delay, script) = setup(vec![
                cts(0xff),
                Step::Spi(vec![Wire::Write(vec![0x10])], Ok(())),
                failing(cts(0xff)),
            ]);
            assert_eq!(
                run!(radio.func_info(&mut delay)),
                Err(Error::Spi(MockError))
            );
            script.done();
        }

        #[test]
        fn typed_identification_does_not_return_data_when_response_cts_is_invalid() {
            for part_info in [true, false] {
                let opcode = if part_info { 0x01 } else { 0x10 };
                let length = if part_info { 8 } else { 6 };
                let (mut radio, mut delay, script) = setup(vec![
                    cts(0xff),
                    Step::Spi(vec![Wire::Write(vec![opcode])], Ok(())),
                    cts(0xff),
                    Step::Spi(
                        vec![
                            Wire::Write(vec![0x44]),
                            Wire::Read(vec![0]),
                            Wire::Read(vec![0; length]),
                        ],
                        Ok(()),
                    ),
                ]);
                if part_info {
                    assert_eq!(
                        run!(radio.part_info(&mut delay)),
                        Err(Error::InvalidResponse)
                    );
                } else {
                    assert_eq!(
                        run!(radio.func_info(&mut delay)),
                        Err(Error::InvalidResponse)
                    );
                }
                script.done();
            }
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
    fn cancelled_configuration_does_not_send_later_commands() {
        let (mut radio, _, script) = setup(vec![
            cts(0xff),
            Step::Spi(vec![Wire::Write(vec![0x34, 3])], Ok(())),
            cts(0),
            Step::Delay(17),
        ]);
        let mut suspended = SuspendedDelay(script.clone());
        cancel_pending(radio.load_configuration(&[2, 0x34, 3, 1, 0, 0], &mut suspended));
        script.done();
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
