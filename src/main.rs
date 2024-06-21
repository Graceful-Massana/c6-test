use std::sync::Mutex;
use core::convert::TryInto;
use esp_idf_svc::hal::{prelude::Peripherals, gpio::*, delay::FreeRtos, modem::Modem};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    http::server::EspHttpServer,
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, EspWifi},
};

use log::*;

use serde::Deserialize;

use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration as WifiConfiguration};
use embedded_svc::{http::{Headers, Method}, io::{Read, Write}};

const SSID: &str = "Enhora";
const PASSWORD: &str = "Intuate@333";
const MAX_LEN: usize = 128;
const STACK_SIZE: usize = 10240;

#[derive(Deserialize)]
struct JsonData<'a> {
    order: &'a str,
    num: &'a str,
}

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let per = Peripherals::take()?;
    let rel1 = per.pins.gpio10;
    let rel1d: AnyIOPin = rel1.downgrade();
    let mutex1 = Mutex::new(rel1d);
    let modem = per.modem;
    let mut server = configure_wifi(modem)?;

    server.fn_handler::<anyhow::Error,_>("/post", Method::Post, |mut req|  {
        
        let len = req.content_len().unwrap_or(0) as usize;
        if len > MAX_LEN {
            req.into_status_response(413)?
                .write_all("Request too big".as_bytes())?;
            return Ok(());
        }

        let mut buf = vec![0; len];
        req.read_exact(&mut buf)?;
        let mut resp = req.into_ok_response()?;
        
        let body_string = String::from_utf8(buf).unwrap();
        let parsed: JsonData = serde_json::from_str(&body_string)?;

        
        if parsed.order == "open_delay" {
            write!(
                resp,
                "Opening relay for 3 seconds",
            )?;
            if parsed.num =="1" {
                let mut lock = mutex1.lock().unwrap();
                let _ = opening_delay(&mut *lock);               
            }
        }
    Ok(())
    })?;
    core::mem::forget(server);
    Ok(())
}

fn opening_delay(pin: &mut AnyIOPin) -> anyhow::Result<()>{
    let mut relay = PinDriver::output(pin)?;
    relay.set_high()?;
    FreeRtos::delay_ms(3000);
    relay.set_low()?;

    Ok(())
}

fn configure_wifi(modem: Modem) -> anyhow::Result<EspHttpServer<'static>> {
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;
    
    let wifi_configuration = WifiConfiguration::Client(ClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: PASSWORD.try_into().unwrap(),
        channel: None,
        ..Default::default()
    });
    wifi.set_configuration(&wifi_configuration)?;
    connect_wifi(&mut wifi)?;

    let server_configuration = esp_idf_svc::http::server::Configuration {
        stack_size: STACK_SIZE,
        ..Default::default()
    };
    
    core::mem::forget(wifi);

    Ok(EspHttpServer::new(&server_configuration)?)
}

fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>) -> anyhow::Result<()> {
    wifi.start()?;
    info!("Wifi started");

    wifi.connect()?;
    info!("Wifi connected");

    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    let info = wifi.wifi().sta_netif().get_ip_info()?;
    info!("IP info: {:?}", info);

    Ok(())
}