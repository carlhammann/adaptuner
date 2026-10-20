#![no_std]
#![no_main]

extern crate alloc;

#[cfg(not(feature = "defmt"))]
use panic_halt as _;

#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use alloc::boxed::Box;

use embassy_executor::Spawner;
use embassy_stm32::{Config, rcc::*, time::Hertz};
use embassy_time::{Instant, Timer};
use embedded_alloc::LlffHeap;

use slint::{
    ComponentHandle, PhysicalSize,
    platform::{
        self,
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    },
};

mod fmt;
mod hardware;
mod ui;

use hardware::display_dma::DisplayDma;
use ui::platform::{DisplayLineBuffer, RenderStats};

const DISPLAY_WIDTH: usize = 320;
const DISPLAY_HEIGHT: usize = 240;
const DISPLAY_FRAMEBUFFER_BYTES: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT * 2;
const HEAP_SIZE: usize = 128 * 1024;

#[global_allocator]
static HEAP: LlffHeap = LlffHeap::empty();

static mut HEAP_MEMORY: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

static mut DISPLAY_FRAMEBUFFER: [u8; DISPLAY_FRAMEBUFFER_BYTES] = [0; DISPLAY_FRAMEBUFFER_BYTES];

slint::include_modules!();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // 1. Heap für Slint und alloc initialisieren.
    unsafe {
        let heap_start = core::ptr::addr_of_mut!(HEAP_MEMORY).cast::<u8>() as usize;

        HEAP.init(heap_start, HEAP_SIZE);
    }

    // 2. RCC-Konfiguration aus Ember übernehmen.
    let mut config = Config::default();

    config.rcc.hsi = Some(HSIPrescaler::DIV1);
    config.rcc.csi = true;

    config.rcc.pll1 = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL50,
        fracn: None,
        divp: Some(PllDiv::DIV2),
        divq: Some(PllDiv::DIV8),
        divr: None,
    });

    config.rcc.pll2 = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL50,
        fracn: None,
        divp: Some(PllDiv::DIV8),
        divq: None,
        divr: None,
    });

    config.rcc.pll3 = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL40,
        fracn: None,
        divp: Some(PllDiv::DIV8),
        divq: None,
        divr: None,
    });

    config.rcc.mux.adcsel = mux::Adcsel::PLL2_P;
    config.rcc.mux.spi123sel = mux::Saisel::PLL3_P;

    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.ahb_pre = AHBPrescaler::DIV2;
    config.rcc.apb1_pre = APBPrescaler::DIV2;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    config.rcc.apb3_pre = APBPrescaler::DIV2;
    config.rcc.apb4_pre = APBPrescaler::DIV2;
    config.rcc.voltage_scale = VoltageScale::Scale1;

    let p = embassy_stm32::init(config);

    // 3. SPI1-DMA-Display initialisieren.
    let mut display = DisplayDma::new(
        p.SPI1,
        p.PA5,
        p.PA7,
        p.DMA1_CH0,
        p.PC4,
        p.PC5,
        p.PB0,
        Hertz(40_000_000),
    );

    if let Err(error) = display.initialize().await {
        defmt::error!("Display initialization failed: {:?}", error);

        loop {
            Timer::after_secs(1).await;
        }
    }

    // 4. Slint-Plattform und Software-Renderer registrieren.
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);

    window.set_size(PhysicalSize::new(
        DISPLAY_WIDTH as u32,
        DISPLAY_HEIGHT as u32,
    ));

    let started_at = Instant::now();

    platform::set_platform(Box::new(ui::platform::SlintPlatform::new(
        window.clone(),
        started_at,
    )))
    .unwrap();

    // 5. Adaptuner-Oberfläche erstellen.
    let app = match AppWindow::new() {
        Ok(app) => app,
        Err(_error) => {
            defmt::error!("Adaptuner UI initialization failed");

            loop {
                Timer::after_secs(1).await;
            }
        }
    };

    app.show().unwrap();

    // 6. Render- und DMA-Loop.
    loop {
        platform::update_timers_and_animations();

        let mut render_stats = RenderStats::default();

        let framebuffer = unsafe {
            core::slice::from_raw_parts_mut(
                core::ptr::addr_of_mut!(DISPLAY_FRAMEBUFFER).cast::<u8>(),
                DISPLAY_FRAMEBUFFER_BYTES,
            )
        };

        let mut display_buffer = DisplayLineBuffer::new(framebuffer, &mut render_stats);

        window.draw_if_needed(|renderer| {
            renderer.render_by_line(&mut display_buffer);
        });

        if let Err(error) = display_buffer.flush(&mut display).await {
            defmt::error!("Display DMA update failed: {:?}", error);

            loop {
                Timer::after_secs(1).await;
            }
        }

        Timer::after_millis(2).await;
    }
}
