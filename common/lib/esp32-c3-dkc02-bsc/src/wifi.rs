// based on https://github.com/ivmarkov/rust-esp32-std-demo/blob/main/src/main.rs

use std::sync::Arc;

use anyhow::bail;
use embedded_svc::wifi::{
    self, AuthMethod, Configuration, AccessPointConfiguration, ClientConfiguration,
    ClientConnectionStatus, ClientIpStatus, ClientStatus, Wifi as _,
};
use esp_idf_svc::{
    netif::EspNetifStack, nvs::EspDefaultNvs, sysloop::EspSysLoopStack, wifi::EspWifi,
};
use std::time::Duration;

#[allow(unused)]
pub struct Wifi {
    esp_wifi: EspWifi,
    netif_stack: Arc<EspNetifStack>,
    sys_loop_stack: Arc<EspSysLoopStack>,
    default_nvs: Arc<EspDefaultNvs>,
}

pub fn wifi(ssid: &str, psk: &str) -> anyhow::Result<Wifi> {
    println!("Wifi config: ssid = {}, psk = {}", ssid, psk);
    let mut auth_method = AuthMethod::WPA2Personal;
    if ssid.len() == 0 {
        anyhow::bail!("missing WiFi name")
    }
    if psk.len() == 0 {
        auth_method = AuthMethod::None;
        println!("Wifi password is empty");
    }
    let netif_stack = Arc::new(EspNetifStack::new()?);
    let sys_loop_stack = Arc::new(EspSysLoopStack::new()?);
    let default_nvs = Arc::new(EspDefaultNvs::new()?);
    let mut wifi = EspWifi::new(
        netif_stack.clone(),
        sys_loop_stack.clone(),
        default_nvs.clone(),
    )?;

    println!("Searching for Wifi network {}", ssid);

    let ap_infos = wifi.scan()?;

    let ours = ap_infos.into_iter().find(|a| a.ssid == ssid);

    let channel = if let Some(ours) = ours {
        println!(
            "Found configured access point {} on channel {}",
            ssid, ours.channel
        );
        Some(ours.channel)
    } else {
        println!(
            "Configured access point {} not found during scanning, will go with unknown channel",
            ssid
        );
        None
    };

    println!("setting Wifi configuration");

    wifi.set_configuration(&Configuration::Mixed(
        ClientConfiguration {
            ssid: ssid.into(),
            password: psk.into(),
            channel,
            auth_method: auth_method,
            ..Default::default()
        },
        AccessPointConfiguration {
            ssid: "aptest".into(),
            channel: channel.unwrap_or(1),
            ..Default::default()
        },
    ))?;

    println!("Wifi configuration set, about to get status");

    wifi.wait_status_with_timeout(Duration::from_secs(20), |status| !status.is_transitional())
        .map_err(|e| anyhow::anyhow!("Unexpected Wifi status: {:?}", e))?;

    let status = wifi.get_status();

    if let wifi::Status(
        ClientStatus::Started(ClientConnectionStatus::Connected(ClientIpStatus::Done(_))),
        _,
    ) = status
    {
        println!("Wifi connected!");
    } else {
        bail!("Unexpected Wifi status: {:?}", status);
    }

    let wifi = Wifi {
        esp_wifi: wifi,
        netif_stack,
        sys_loop_stack,
        default_nvs,
    };

    Ok(wifi)
}
