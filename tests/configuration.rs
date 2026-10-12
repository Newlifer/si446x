use si446x::{Configuration, ConfigurationError as E};

fn error(bytes: &[u8], expected: E) {
    assert_eq!(Configuration::new(bytes).err(), Some(expected));
}

#[test]
fn parses_multiple_commands_and_preserves_original_slice() {
    let bytes = [5, 0x11, 0x20, 1, 0, 0x03, 2, 0x34, 3, 1, 0x17, 0];
    let result = Configuration::new(&bytes);
    assert!(result.is_ok());
    if let Ok(config) = result {
        assert_eq!(config.as_bytes(), bytes);
        assert_eq!(
            config.commands().collect::<Vec<_>>(),
            vec![&[0x11, 0x20, 1, 0, 0x03][..], &[0x34, 3], &[0x17],]
        );
    }
}

#[test]
fn empty_configuration_is_a_single_terminator() {
    let result = Configuration::new(&[0]);
    assert!(result.is_ok());
    if let Ok(config) = result {
        let mut commands = config.commands();
        assert_eq!(commands.next(), None);
        assert_eq!(commands.next(), None);
    }
    error(&[], E::MissingTerminator);
}

#[test]
fn requires_terminator_and_rejects_trailing_data() {
    error(&[1, 0], E::MissingTerminator); // NOP payload, not a terminator.
    error(&[0, 0], E::TrailingData { offset: 1 });
    error(&[1, 0, 0, 0xff], E::TrailingData { offset: 3 });
}

#[test]
fn checks_lengths_and_truncation_with_record_offsets() {
    error(
        &[17],
        E::InvalidLength {
            offset: 0,
            length: 17,
        },
    );
    error(
        &[255],
        E::InvalidLength {
            offset: 0,
            length: 255,
        },
    );
    error(&[1, 0, 3, 0x11], E::Truncated { offset: 2 });
    error(&[1], E::Truncated { offset: 0 });
}

#[test]
fn rejects_bad_known_command_lengths_and_parameters() {
    for command in [
        vec![0x11, 0x20, 0, 0],
        vec![0x11, 0x20, 2, 0, 1],
        vec![0x11, 0x20, 2, 255, 1, 2],
        vec![0x12, 0x20, 17, 0],
        vec![0x12, 0x20, 2, 255],
        vec![0x34, 4],
        vec![0x34, 0x83],
        vec![0x34],
        vec![0x01, 0],
        vec![0x10, 0],
        vec![0x33, 0],
        vec![0x02, 1, 1, 1, 0xc9, 0xc3, 0x80],
        vec![0x02, 0x41, 0, 1, 0xc9, 0xc3, 0x80],
        vec![0x02, 1, 0, 0, 0, 0, 0],
        vec![0x13, 0, 0, 0, 0, 0, 0],
        vec![0x13, 30, 0, 0, 0, 0, 0, 0],
        vec![0x13, 0x80, 0, 0, 0, 0, 0, 0],
        vec![0x13, 0, 0, 0, 0, 5, 0, 0],
        vec![0x13, 0, 0, 0, 0, 0, 39, 0],
        vec![0x13, 0, 0, 0, 0, 0, 0, 1],
    ] {
        let mut bytes = vec![command.len() as u8];
        bytes.extend(command);
        bytes.push(0);
        error(&bytes, E::InvalidCommand { offset: 0 });
    }
}

#[test]
fn accepts_maximum_property_record_and_full_read_range() {
    let mut bytes = vec![16, 0x11, 0x20, 12, 244];
    bytes.extend([0xff; 12]);
    bytes.extend([4, 0x12, 0x20, 16, 240, 0]);
    assert!(Configuration::new(&bytes).is_ok());
}

#[test]
fn rejects_direct_operations_that_cannot_use_command_transport() {
    for opcode in [0x44, 0x50, 0x51, 0x53, 0x57, 0x66, 0x77] {
        error(&[1, opcode, 0], E::InvalidCommand { offset: 0 });
    }
}

#[test]
fn opaque_commands_only_receive_framing_validation() {
    assert!(Configuration::new(&[2, 0x17, 0xff, 0]).is_ok());
}

#[test]
fn arbitrary_short_inputs_do_not_panic() {
    for first in 0..=255 {
        for second in 0..=255 {
            if let Ok(config) = Configuration::new(&[first, second, 0]) {
                assert!(config.commands().count() <= 1);
            }
        }
    }
}
