//! This example runs on the RAK4631 WisBlock, which has an nRF52840 MCU and Semtech Sx126x radio.
//! Other nrf/sx126x combinations may work with appropriate pin modifications.
//! It demonstrates LoRaWAN join functionality.
#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pin as _, Pull};
use embassy_nrf::rng::Rng;
use embassy_nrf::{bind_interrupts, peripherals, rng, spim};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::GenericSx126xInterfaceVariant;
use lora_phy::lorawan_radio::LorawanRadio;
use lora_phy::sx126x::{self, Sx1262, Sx126x, TcxoCtrlVoltage};
use lora_phy::LoRa;
use lorawan_device::async_device::{region, Device, EmbassyTimer, JoinMode};
use lorawan_device::default_crypto::DefaultFactory as Crypto;
use lorawan_device::{AppEui, AppKey, DevEui};
use {defmt_rtt as _, panic_probe as _};

// warning: set these appropriately for the region
const LORAWAN_REGION: region::Region = region::Region::EU868;
const MAX_TX_POWER: u8 = 14;

// Load optional override values for EUIs and APPKEY generated by build.rs from
// environment values
include!(concat!(env!("OUT_DIR"), "/lorawan_keys.rs"));

// Fallback values
const DEFAULT_DEVEUI: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
const DEFAULT_APPEUI: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
const DEFAULT_APPKEY: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

bind_interrupts!(struct Irqs {
    SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1 => spim::InterruptHandler<peripherals::TWISPI1>;
    RNG => rng::InterruptHandler<peripherals::RNG>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());

    let nss = Output::new(p.P1_10.degrade(), Level::High, OutputDrive::Standard);
    let reset = Output::new(p.P1_06.degrade(), Level::High, OutputDrive::Standard);
    let dio1 = Input::new(p.P1_15.degrade(), Pull::Down);
    let busy = Input::new(p.P1_14.degrade(), Pull::Down);
    let rf_switch_rx = Output::new(p.P1_05.degrade(), Level::Low, OutputDrive::Standard);
    let rf_switch_tx = Output::new(p.P1_07.degrade(), Level::Low, OutputDrive::Standard);

    let mut spi_config = spim::Config::default();
    spi_config.frequency = spim::Frequency::M16;
    let spim = spim::Spim::new(p.TWISPI1, Irqs, p.P1_11, p.P1_13, p.P1_12, spi_config);
    let spi = ExclusiveDevice::new(spim, nss, Delay);

    let config = sx126x::Config {
        chip: Sx1262,
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
        use_dcdc: true,
        rx_boost: false,
    };
    let iv = GenericSx126xInterfaceVariant::new(reset, dio1, busy, Some(rf_switch_rx), Some(rf_switch_tx)).unwrap();
    let lora = LoRa::new(Sx126x::new(spi, iv, config), true, Delay).await.unwrap();

    let radio: LorawanRadio<_, _, MAX_TX_POWER> = lora.into();
    let region: region::Configuration = region::Configuration::new(LORAWAN_REGION);
    let mut device: Device<_, Crypto, _, _> = Device::new(region, radio, EmbassyTimer::new(), Rng::new(p.RNG, Irqs));

    defmt::info!("Joining LoRaWAN network");

    let resp = device
        .join(&JoinMode::OTAA {
            deveui: DevEui::from(DEVEUI.unwrap_or(DEFAULT_DEVEUI)),
            appeui: AppEui::from(APPEUI.unwrap_or(DEFAULT_APPEUI)),
            appkey: AppKey::from(APPKEY.unwrap_or(DEFAULT_APPKEY)),
        })
        .await
        .unwrap();

    info!("LoRaWAN network joined: {:?}", resp);

    #[cfg(feature = "teleprobe")]
    cortex_m::asm::bkpt();
}
