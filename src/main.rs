#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use esp_hal::clock::CpuClock;
use esp_hal::rmt::Rmt;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal_smartled::{RmtSmartLeds, buffer_size, color_order};
use esp_println as _;

use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_time::{Duration, Ticker};
use static_cell::StaticCell;

use smart_leds::{RGB8, SmartLedsWrite, brightness, gamma};

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    error!("{}", panic_info);
    loop {}
}

const FPS: usize = 30;
const BYTES_PER_FRAME: usize = 8;
const FRAME_COUNT: usize = FRAMES.len() / BYTES_PER_FRAME;
const LED_COUNT: usize = BYTES_PER_FRAME * BYTES_PER_FRAME;

const LED_BRIGHTNESS: u8 = 1;
const BG: RGB8 = RGB8 {
    r: 255,
    g: 255,
    b: 255,
};
const FG: RGB8 = RGB8 {
    r: 16,
    g: 16,
    b: 16,
};
const PALETTE: [RGB8; 2] = [FG, BG];

type LedColor = RGB8;

static FRAMES: &[u8] = include_bytes!("../bad_apple_8x8.bin");
static BUFFER: StaticCell<[RGB8; LED_COUNT]> = StaticCell::new();

esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);
    let _ = spawner;

    info!("[MAIN] app started! ");

    let freq = Rate::from_mhz(80);
    let rmt = Rmt::new(peripherals.RMT, freq).expect("Failed to initialize RMT");

    let mut led =
        RmtSmartLeds::<{ buffer_size::<LedColor>(LED_COUNT) }, _, LedColor, color_order::Grb>::new(
            esp_hal_smartled::WS2811_TIMING,
            rmt.channel0,
            peripherals.GPIO14,
            freq,
        )
        .expect("Failed to initialize led");

    let mut ticker = Ticker::every(Duration::from_millis((1000_f64 / (FPS as f64)) as u64));

    let buffer: &'static mut [RGB8; LED_COUNT] =
        BUFFER.init([RGB8 { r: 255, g: 0, b: 0 }; LED_COUNT]);

    loop {
        for frame in 0..FRAME_COUNT {
            *buffer = [PALETTE[0]; LED_COUNT];
            for (i, frame_in_buffer) in buffer.iter_mut().enumerate().take(LED_COUNT) {
                if pixel_on(frame, i) {
                    *frame_in_buffer = PALETTE[1];
                }
            }

            if let Err(error) = led.write(brightness(gamma(buffer.iter().cloned()), LED_BRIGHTNESS))
            {
                error!("[LED] failed to write {:#?}", error);
            };
            ticker.next().await;
        }
    }
}

#[inline]
fn pixel_on(frame: usize, n: usize) -> bool {
    let byte = FRAMES[frame * BYTES_PER_FRAME + n / 8];
    (byte >> (n % 8)) & 1 != 0
}
