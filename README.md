Rust driver for Si446x radios [![Status Ventis][status-ventis]][andivionian-status-classifier]
=========================================

This is a platform agnostic Rust driver for the Si446x radio trancivers using the [`embedded-hal`] traits and SPI.

This driver allows you to:

- Reset, power up and identify the radio.
- Read/write properties and load WDS configurations.
- Configure GPIO and control the radio state.

Supports `no_std`, blocking and optional async operation.

## The devices

Silicon Labs Si446x radio transceivers. Hardware validation is pending.

## Usage example (blocking, low level api)

Wiring: `CS` and `SDN` are plain GPIO outputs, data goes over SPI.

```rust
let spi_device = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
let mut radio = Si446x::new(spi_device, sdn);
```

**Sequence:**

1. Reset via the SDN pin:

```rust
radio.reset(&mut delay)?;
```

2. Load the profile generated in WDS (includes `POWER_UP`):

```rust
radio.load_configuration(RADIO_CONFIGURATION, &mut delay)?;
```

3. Check communication with the chip:

```rust
radio.part_info(&mut delay)?;
```

4. Switch state (`Ready` / `Rx` / `Tx`):

```rust
radio.change_state(State::Rx, &mut delay)?;
```

5. Fine-tune on top of the profile — properties and GPIO:

```rust
radio.set_property(0x22, 0x01, &[0x1F], &mut delay)?; // PA power
radio.gpio_pin_cfg(cfg, &mut delay)?;
```

6. Transmission:

```rust
radio.change_state(State::Tx, &mut delay)?;
// data: into the TX FIFO, or direct mode — toggling GPIO0
radio.change_state(State::Ready, &mut delay)?;
```

7. Status check:

```rust
let state = radio.request_device_state(&mut delay)?;
```

## License

Licensed under MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

[andivionian-status-classifier]: https://andivionian.fornever.me/v1/#status-ventis-
[status-ventis]: https://img.shields.io/badge/status-ventis-yellow.svg
