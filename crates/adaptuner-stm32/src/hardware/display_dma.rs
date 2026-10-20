use embassy_stm32::{
    Peri, bind_interrupts,
    dma::InterruptHandler,
    gpio::{Level, Output, Speed},
    peripherals::{DMA1_CH0, PA5, PA7, PB0, PC4, PC5, SPI1},
    spi::{self, Config as SpiConfig, Spi},
    time::Hertz,
};
use embassy_time::Timer;

bind_interrupts!(pub struct Irqs {
    DMA1_STREAM0 => InterruptHandler<DMA1_CH0>;
});

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 240;
pub const BYTES_PER_PIXEL: usize = 2;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT * BYTES_PER_PIXEL;

const CHUNK_BYTES: usize = 4096;

#[derive(Debug, defmt::Format)]
pub enum DisplayError {
    InvalidBounds,
    InvalidBufferLength,
    Spi(spi::Error),
}

impl From<spi::Error> for DisplayError {
    fn from(error: spi::Error) -> Self {
        Self::Spi(error)
    }
}

pub struct DisplayDma<'d> {
    spi: Spi<'d, embassy_stm32::mode::Async, spi::mode::Master>,
    cs: Output<'d>,
    dc: Output<'d>,
    rst: Output<'d>,
}

impl<'d> DisplayDma<'d> {
    pub fn new(
        spi_peripheral: Peri<'d, SPI1>,
        sck: Peri<'d, PA5>,
        mosi: Peri<'d, PA7>,
        dma: Peri<'d, DMA1_CH0>,
        cs_pin: Peri<'d, PC4>,
        dc_pin: Peri<'d, PC5>,
        reset_pin: Peri<'d, PB0>,
        frequency: Hertz,
    ) -> Self {
        let mut spi_config = SpiConfig::default();
        spi_config.frequency = frequency;

        let spi = Spi::new_txonly(spi_peripheral, sck, mosi, dma, Irqs, spi_config);

        let actual_frequency = spi.get_current_config().frequency.0;

        defmt::info!(
            "SPI requested: {} Hz, actual: {} Hz",
            frequency.0,
            actual_frequency
        );

        Self {
            spi,
            cs: Output::new(cs_pin, Level::High, Speed::VeryHigh),
            dc: Output::new(dc_pin, Level::Low, Speed::VeryHigh),
            rst: Output::new(reset_pin, Level::High, Speed::Low),
        }
    }

    pub async fn initialize(&mut self) -> Result<(), DisplayError> {
        // Hardware-Reset des ST7789.
        self.rst.set_low();
        Timer::after_millis(20).await;

        self.rst.set_high();
        Timer::after_millis(150).await;

        // Sleep Out.
        self.command(0x11, &[]).await?;
        Timer::after_millis(10).await;

        // Landscape und RGB-Farbreihenfolge.
        self.command(0x36, &[0x60]).await?;

        // Farb-Invertierung wie in der funktionierenden Slint-Demo.
        self.command(0x21, &[]).await?;

        // RGB565.
        self.command(0x3A, &[0x55]).await?;
        Timer::after_millis(10).await;

        // Normal Mode.
        self.command(0x13, &[]).await?;
        Timer::after_millis(10).await;

        // Display On.
        self.command(0x29, &[]).await?;
        Timer::after_millis(120).await;

        Ok(())
    }

    /// Zeichnet einen vollständigen RGB565-Frame im Landscape-Format 320 × 240.
    ///
    /// Die Pixel müssen zeilenweise und als High-Byte/Low-Byte vorliegen.
    pub async fn write_frame(&mut self, pixels: &[u8]) -> Result<(), DisplayError> {
        if pixels.len() != FRAME_BYTES {
            return Err(DisplayError::InvalidBufferLength);
        }

        self.write_rect(0, 0, WIDTH as u16, HEIGHT as u16, pixels)
            .await
    }

    /// Aktualisiert einen rechteckigen Bereich.
    ///
    /// `pixels` enthält ausschließlich die Pixel dieses Rechtecks,
    /// zeilenweise im RGB565-Format.
    pub async fn write_rect(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        pixels: &[u8],
    ) -> Result<(), DisplayError> {
        let x_end = x as usize + width as usize;
        let y_end = y as usize + height as usize;

        if width == 0 || height == 0 || x_end > WIDTH || y_end > HEIGHT {
            return Err(DisplayError::InvalidBounds);
        }

        let expected_bytes = width as usize * height as usize * BYTES_PER_PIXEL;

        if pixels.len() != expected_bytes {
            return Err(DisplayError::InvalidBufferLength);
        }

        // Column Address Set: inklusive Endkoordinate.
        self.command(
            0x2A,
            &[
                (x >> 8) as u8,
                x as u8,
                ((x_end - 1) >> 8) as u8,
                (x_end - 1) as u8,
            ],
        )
        .await?;

        // Row Address Set: inklusive Endkoordinate.
        self.command(
            0x2B,
            &[
                (y >> 8) as u8,
                y as u8,
                ((y_end - 1) >> 8) as u8,
                (y_end - 1) as u8,
            ],
        )
        .await?;

        // RAM Write starten. CS bleibt während der Pixelübertragung Low.
        self.cs.set_low();
        self.dc.set_low();

        if let Err(error) = self.spi.write(&[0x2Cu8]).await {
            self.cs.set_high();
            return Err(DisplayError::Spi(error));
        }

        self.dc.set_high();

        for chunk in pixels.chunks(CHUNK_BYTES) {
            if let Err(error) = self.spi.write(chunk).await {
                self.cs.set_high();
                return Err(DisplayError::Spi(error));
            }
        }

        self.cs.set_high();

        Ok(())
    }

    async fn command(&mut self, cmd: u8, args: &[u8]) -> Result<(), DisplayError> {
        self.cs.set_low();
        self.dc.set_low();

        if let Err(error) = self.spi.write(&[cmd]).await {
            self.cs.set_high();
            return Err(DisplayError::Spi(error));
        }

        if !args.is_empty() {
            self.dc.set_high();

            if let Err(error) = self.spi.write(args).await {
                self.cs.set_high();
                return Err(DisplayError::Spi(error));
            }
        }

        self.cs.set_high();

        Ok(())
    }
}
