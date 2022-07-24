use core::str;
use std::{
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};

use bsc::{temp_sensor::BoardTempSensor, wifi::wifi};
use embedded_svc::{
    http::{
        server::{registry::Registry, Response, ResponseWrite},
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

    server.set_inline_handler("/led", Method::Get, |request, response| {
        func_blink_led_green();
        let html = index_html();
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

fn func_blink_led_green() {
    let now = get_cur_time();
    println!("{} ~ Got a request path: /led", now);
    _do_blink_led_green().expect("");
}

fn _do_blink_led_green() -> anyhow::Result<()> {
    println!("Configuring output channel");

    let peripherals = Peripherals::take().unwrap();
    let config = TimerConfig::default().frequency(25.kHz().into());
    let timer = Timer::new(peripherals.ledc.timer0, &config)?;
    let mut channel = Channel::new(peripherals.ledc.channel0, &timer, peripherals.pins.gpio4)?;

    println!("Starting duty-cycle loop");

    let max_duty = channel.get_max_duty();
    for numerator in [0, 1, 2, 3, 4, 5].iter().cycle() {
        println!("Duty {}/5", numerator);
        channel.set_duty(max_duty * numerator / 5)?;
        FreeRtos.delay_ms(2000)?;
    }
    Ok(())

    /*
    loop {
        FreeRtos.delay_ms(1000)?;
    }
    */
}
