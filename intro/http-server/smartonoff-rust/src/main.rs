use core::str;
use std::{
    thread,
    thread::sleep,
    time::Duration,
};

use bsc::{wifi::wifi};
use embedded_svc::{
    http::{
        server::{registry::Registry, Request, Response, ResponseWrite},
        Method,
    },
    io::Write,
};

use esp32_c3_dkc02_bsc as bsc;
use esp_idf_svc::http::server::{Configuration, EspHttpServer};
use esp_idf_sys as _; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported
use std::time::{SystemTime, UNIX_EPOCH};

use embedded_hal::delay::blocking::DelayUs;

use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::ledc::{config::TimerConfig, Channel, Timer};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::prelude::*;

#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
}

static mut G_LED_ON: bool = false;

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();

    let _wifi = wifi(CONFIG.wifi_ssid, CONFIG.wifi_psk)?;

    let server_config = Configuration::default();
    let mut server = EspHttpServer::new(&server_config)?;
    server.set_inline_handler("/", Method::Get, |request, response| {
        let html = index_html();
        let mut writer = response.into_writer(request)?;
        writer.do_write_all(html.as_bytes())?;
        writer.complete()
    })?;

    thread::spawn(|| -> anyhow::Result<()> {
        let peripherals = Peripherals::take().unwrap();
        let config = TimerConfig::default().frequency(25.kHz().into());
        let timer = Timer::new(peripherals.ledc.timer0, &config)?;
        let mut channel = Channel::new(peripherals.ledc.channel0, &timer, peripherals.pins.gpio4)?;
        let max_duty = channel.get_max_duty();
        let max_num = 33;
        let duty_interval = 2000 / max_num; // 2s
        loop {
            for numerator in 0..(max_num + 1) {
                unsafe {
                    if !G_LED_ON {
                        channel.set_duty(0)?;
                        break;
                    }
                }
                channel.set_duty(max_duty * numerator / max_num)?;
                FreeRtos.delay_ms(duty_interval)?;
            }
            for numerator in (0..(max_num + 1)).rev() {
                unsafe {
                    if !G_LED_ON {
                        channel.set_duty(0)?;
                        break;
                    }
                }
                channel.set_duty(max_duty * numerator / max_num)?;
                FreeRtos.delay_ms(duty_interval)?;
            }
            FreeRtos.delay_ms(500)?;
        }
    });

    server.set_inline_handler("/led", Method::Get, |request, response| {
        let now = get_cur_time();
        let query_str = request.query_string().to_string();
        println!(
            "{} ~ Got a request path: /led, query_string = {}",
            now, query_str
        );
        let html;
        if query_str == "off" {
            unsafe {
                G_LED_ON = false;
            }
            html = templated(format!("{} ~ The LED is off.", now));
        } else if query_str == "on" {
            unsafe {
                G_LED_ON = true;
            }
            html = templated(format!("{} ~ The LED is fading in/out ...", now));
        } else {
            html = templated(format!("{} ~ Invalid cmd!", now));
        }
        let mut writer = response.into_writer(request)?;
        writer.do_write_all(html.as_bytes())?;
        writer.complete()
    })?;

    println!("server awaiting connection");

    loop {
        sleep(Duration::from_millis(1000));
    }
}

fn templated(content: impl AsRef<str>) -> String {
    format!(
        r#"
<!DOCTYPE html>
<html>
    <head>
        <meta charset="utf-8">
        <title>esp-rs web server</title>
    </head>
    <body>
        {}
    </body>
</html>
"#,
        content.as_ref()
    )
}

fn get_cur_time() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_secs()
}

fn index_html() -> String {
    let now = get_cur_time();
    println!("{} ~ Got a request path: /", now);
    templated(format!("{} ~ Hello from mcu!", now))
}
