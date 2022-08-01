use core::str;
use std::{
    sync::{mpsc, mpsc::Sender, Arc, Mutex},
    thread,
    thread::sleep,
    time::Duration,
};

use bsc::{temp_sensor::BoardTempSensor, wifi::wifi};
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

    let led_thread_handle = Some(spawn_led_thread());
    server.set_inline_handler("/led", Method::Get, move |request, response| {
        let now = get_cur_time();
        let query_str = request.query_string().to_string();
        println!(
            "{} ~ Got a request path: /led, query_string = {}",
            now, query_str
        );
        let html;
        if query_str == "off" {
            if let Some(ref led_thread_handle) = led_thread_handle {
                println!("{} ~ Try to drop the LED thread ...", now);
                drop(led_thread_handle);
            }
            html = templated(format!("{} ~ The LED is off.", now));
        } else if query_str == "on" {
            if let Some(ref led_thread_handle) = led_thread_handle {
                led_thread_handle.send(query_str).unwrap();
                html = templated(format!("{} ~ The LED is fading in / out ...", now));
            } else {
                let tmp_msg = "The LED thread has been stopped!";
                println!("{}", tmp_msg);
                html = templated(format!("{} ~ {}", now, tmp_msg));
            };
        }
        else {
            html = templated(format!("{} ~ Invalid cmd!", now));
        }
        let mut writer = response.into_writer(request)?;
        writer.do_write_all(html.as_bytes())?;
        writer.complete()
    })?;

    let temp_sensor_main = Arc::new(Mutex::new(BoardTempSensor::new_taking_peripherals()));
    let temp_sensor = temp_sensor_main.clone();

    server.set_inline_handler("/temperature", Method::Get, move |request, response| {
        let temp_val = temp_sensor.lock().unwrap().read_owning_peripherals();
        let html = func_temperature(temp_val);
        let mut writer = response.into_writer(request)?;
        writer.do_write_all(html.as_bytes())?;
        writer.complete()
    })?;

    println!("server awaiting connection");

    loop {
        sleep(Duration::from_millis(1000));
    }
}

fn spawn_led_thread() -> Sender<String> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let peripherals = Peripherals::take().unwrap();
        let config = TimerConfig::default().frequency(25.kHz().into());
        let timer = Timer::new(peripherals.ledc.timer0, &config).unwrap();
        let mut channel = Channel::new(peripherals.ledc.channel0, &timer, peripherals.pins.gpio4).unwrap();
        let max_duty = channel.get_max_duty();
        match rx.recv() {
            Ok(msg) => {
                println!("rx.received msg = {}", msg);
                if msg == "on" {
                    println!("The LED lights start to fade in and fade out ...");
                    let max_num = 100;
                    loop {
                        for numerator in 0..(max_num + 1) {
                            channel.set_duty(max_duty * numerator / max_num).unwrap();
                            FreeRtos.delay_ms(20).unwrap();
                        }
                        for numerator in (0..(max_num + 1)).rev() {
                            channel.set_duty(max_duty * numerator / max_num).unwrap();
                            FreeRtos.delay_ms(20).unwrap();
                        }
                        FreeRtos.delay_ms(500).unwrap();
                    }
                }
            }
            Err(_) => {
                let now = get_cur_time();
                println!("{} ~ Didn't receive any msg.", now);
            }
        }
    });
    tx
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

fn func_temperature(val: f32) -> String {
    let now = get_cur_time();
    println!("{} ~ Got a request path: /temperature", now);
    templated(format!("{} ~ chip temperature: {:.2}°C", now, val))
}
