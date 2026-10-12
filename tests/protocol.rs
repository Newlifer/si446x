use si446x::{BootFunction, FuncInfo, PartInfo, PowerUp, protocol::command};

use si446x::{
    DeviceState, DriveStrength, GetProperty, GpioPinConfig, GpioPinStatus, PinConfig, PinStatus,
    SetProperty, State,
};

#[test]
fn property_commands_encode_counts_and_only_used_data() {
    let encoded = SetProperty {
        group: 0x20,
        start: 0x10,
        values: &[0xaa, 0xbb],
    }
    .encode();
    assert!(encoded.is_some());
    if let Some((bytes, length)) = encoded {
        assert_eq!(length, 6);
        assert_eq!(&bytes[..length], &[0x11, 0x20, 2, 0x10, 0xaa, 0xbb]);
    }
    assert_eq!(
        GetProperty {
            group: 0x20,
            start: 240,
            count: 16
        }
        .encode(),
        Some([0x12, 0x20, 16, 240])
    );
}

#[test]
fn property_ranges_validate_count_and_end_of_group() {
    for (start, count, valid) in [
        (0, 0, false),
        (0, 1, true),
        (244, 12, true),
        (245, 12, false),
        (0, 13, false),
        (255, 1, true),
        (255, 2, false),
    ] {
        assert_eq!(
            SetProperty {
                group: 0,
                start,
                values: &vec![0; count]
            }
            .encode()
            .is_some(),
            valid
        );
    }
    for (start, count, valid) in [
        (0, 0, false),
        (240, 16, true),
        (241, 16, false),
        (0, 17, false),
        (255, 1, true),
        (255, 2, false),
    ] {
        assert_eq!(
            GetProperty {
                group: 0,
                start,
                count
            }
            .encode()
            .is_some(),
            valid
        );
    }
}

#[test]
fn gpio_encodes_pin_modes_pullups_and_drive_strength() {
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
        drive_strength: DriveStrength::Low,
    };
    assert_eq!(config.encode(), Some([0x13, 4, 0x48, 2, 3, 39, 11, 0x60]));
    assert_eq!(
        GpioPinConfig::default().encode(),
        Some([0x13, 0, 0, 0, 0, 0, 0, 0])
    );
}

#[test]
fn gpio_checks_mode_sets_per_pin_and_reserved_bits() {
    for mode in 0..=255 {
        let pin = PinConfig {
            mode,
            pull_up: true,
        };
        let mut config = GpioPinConfig::default();
        config.gpio[0] = pin;
        assert_eq!(config.encode().is_some(), matches!(mode, 0..=29 | 31..=39));
        config = GpioPinConfig {
            nirq: pin,
            ..GpioPinConfig::default()
        };
        assert_eq!(
            config.encode().is_some(),
            matches!(mode, 0..=4 | 7 | 8 | 11 | 12 | 15..=27 | 29 | 31 | 39)
        );
        config = GpioPinConfig {
            sdo: pin,
            ..GpioPinConfig::default()
        };
        assert_eq!(
            config.encode().is_some(),
            matches!(mode, 0..=5 | 7 | 8 | 11 | 12 | 14..=27)
        );
    }
}

#[test]
fn gpio_reply_uses_bit_seven_for_level_and_ignores_undefined_bits() {
    let status = GpioPinStatus::from_bytes([0xc4, 8, 0x82, 3, 0xa7, 0xcb, 0xff]);
    assert_eq!(
        status.gpio[0],
        PinStatus {
            mode: 4,
            high: true
        }
    );
    assert_eq!(
        status.gpio[1],
        PinStatus {
            mode: 8,
            high: false
        }
    );
    assert_eq!(
        status.nirq,
        PinStatus {
            mode: 39,
            high: true
        }
    );
    assert_eq!(
        status.sdo,
        PinStatus {
            mode: 11,
            high: true
        }
    );
    assert_eq!(status.drive_strength, DriveStrength::Low);
    assert_eq!(
        PinStatus::from_byte(0x7f),
        PinStatus {
            mode: 63,
            high: false
        }
    );
}

#[test]
fn device_state_masks_undefined_bits_and_preserves_unknown_codes() {
    let status = DeviceState::from_bytes([0xf4, 0x91]);
    assert_eq!(status.main_state, 4);
    assert_eq!(status.channel, 0x91);
    assert_eq!(status.state(), Some(State::Ready));
    assert_eq!(DeviceState::from_bytes([0xff, 0]).state(), None);
    assert_eq!(command::RequestDeviceState.encode(), [0x33]);
    for (state, byte) in [
        (State::NoChange, 0),
        (State::Sleep, 1),
        (State::SpiActive, 2),
        (State::Ready, 3),
        (State::TxTune, 5),
        (State::RxTune, 6),
        (State::Tx, 7),
        (State::Rx, 8),
    ] {
        assert_eq!(command::ChangeState(state).encode(), [0x34, byte]);
    }
}

#[test]
fn default_power_up_matches_30_mhz_application_boot() {
    assert_eq!(
        PowerUp::default().encode(),
        Some([0x02, 0x01, 0x00, 0x01, 0xc9, 0xc3, 0x80])
    );
}

#[test]
fn power_up_encodes_function_and_patch_without_reserved_bits() {
    for (function, patch, boot_options) in [
        (BootFunction::Bootloader, false, 0x00),
        (BootFunction::Bootloader, true, 0x80),
        (BootFunction::MainApplication, false, 0x01),
        (BootFunction::MainApplication, true, 0x81),
    ] {
        let config = PowerUp {
            function,
            patch,
            xo_frequency_hz: 26_000_000,
        };
        assert_eq!(
            config.encode(),
            Some([0x02, boot_options, 0, 0x01, 0x8c, 0xba, 0x80])
        );
    }
}

#[test]
fn power_up_validates_inclusive_frequency_limits() {
    for (frequency, expected) in [
        (25_000_000, Some([0x02, 1, 0, 0x01, 0x7d, 0x78, 0x40])),
        (32_000_000, Some([0x02, 1, 0, 0x01, 0xe8, 0x48, 0x00])),
        (0, None),
        (24_999_999, None),
        (32_000_001, None),
        (u32::MAX, None),
    ] {
        assert_eq!(
            PowerUp {
                xo_frequency_hz: frequency,
                ..PowerUp::default()
            }
            .encode(),
            expected
        );
    }
}

#[test]
fn identification_requests_have_only_the_opcode() {
    assert_eq!(command::PartInfo.encode(), [0x01]);
    assert_eq!(command::FuncInfo.encode(), [0x10]);
}

#[test]
fn part_info_decodes_every_field_without_cts() {
    assert_eq!(
        PartInfo::from_bytes([0x11, 0x44, 0x60, 0x22, 0x12, 0x34, 0x33, 0x06]),
        PartInfo {
            chip_revision: 0x11,
            part: 0x4460,
            part_build: 0x22,
            id: 0x1234,
            customer: 0x33,
            rom_id: 0x06,
        }
    );
}

#[test]
fn func_info_decodes_every_field_without_cts() {
    assert_eq!(
        FuncInfo::from_bytes([0x03, 0x04, 0x05, 0xab, 0xcd, 0x01]),
        FuncInfo {
            revision_external: 3,
            revision_branch: 4,
            revision_internal: 5,
            patch: 0xabcd,
            function: 1,
        }
    );
    assert_eq!(FuncInfo::from_bytes([0; 6]).patch, 0);
}

#[test]
fn identification_preserves_unknown_values() {
    let part = PartInfo::from_bytes([0xff; 8]);
    assert_eq!(part.part, 0xffff);
    assert_eq!(part.id, 0xffff);
    assert_eq!(part.rom_id, 0xff);
    let func = FuncInfo::from_bytes([0xff; 6]);
    assert_eq!(func.patch, 0xffff);
    assert_eq!(func.function, 0xff);
}
