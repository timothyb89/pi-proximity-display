# pi-proximity-display

This is a tiny daemon to control Pi DSI displays using a VCNL4010 proximity
sensor. It will turn on the display automatically when proximity is detected,
and turn it off automatically after a period once cleared.

Additionally, it can optionally manage the display backlight based on the
sensor's built-in ambient light sensor.

The [`vcnl4010`](./vcnl4010/) directory also contains a small Rust crate for
working with the sensor module in other applications.

## Building

Assuming a modern Pi running 64-bit Raspberry Pi OS:

```bash
cross build --target aarch64-unknown-linux-gnu --target-dir $(pwd)/target-cross --release
```

## Home Assistant Dashboard

This project was used to build a Home Assistant dashboard that automatically
manages its display power.

### BOM

#### Hardware

* Raspberry Pi 5. The Pi 4 is fine too, but Home Assistant dashboards can run a
  bit poorly on the higher resolution Pi Touch Display 2.
* Pi Touch Display 2: https://www.adafruit.com/product/6079
* VCNL4010: https://www.adafruit.com/product/466
* Case of choice. I'm using a heavily modified version of
  https://learn.adafruit.com/pi-wall-mount. Unfortunately the source models have
  an unspecified license so I'm not able to share the changes.

The official V1 display works fine too, but is very low resolution and looks
very bad off axis.

#### Software

* This project
* TouchKio: https://github.com/leukipp/touchkio
