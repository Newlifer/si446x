use si446x::{BootFunction, FuncInfo, PartInfo, PowerUp, protocol::command};

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
