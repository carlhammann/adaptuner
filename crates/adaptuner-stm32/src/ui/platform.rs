extern crate alloc;

use alloc::rc::Rc;

use embassy_time::Instant;
use slint::{
    PlatformError,
    platform::{
        Platform, WindowAdapter,
        software_renderer::{LineBufferProvider, MinimalSoftwareWindow, Rgb565Pixel},
    },
};

use crate::hardware::display_dma::{DisplayDma, DisplayError};

pub const DISPLAY_WIDTH: usize = 320;
pub const DISPLAY_HEIGHT: usize = 240;
pub const DISPLAY_BATCH_HEIGHT: usize = 16;

const DISPLAY_BATCH_BYTES: usize = DISPLAY_WIDTH * DISPLAY_BATCH_HEIGHT * 2;

#[derive(Default)]
pub struct RenderStats {
    pub render_us: u64,
    pub framebuffer_copy_us: u64,
    pub process_line_us: u64,
    pub copy_us: u64,
    pub transfer_us: u64,
    pub flush_overhead_us: u64,
    pub lines: u32,
    pub pixels: u32,
    pub batches: u32,
}

pub struct SlintPlatform {
    window: Rc<MinimalSoftwareWindow>,
    started_at: Instant,
}

impl SlintPlatform {
    pub fn new(window: Rc<MinimalSoftwareWindow>, started_at: Instant) -> Self {
        Self { window, started_at }
    }
}

impl Platform for SlintPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> core::time::Duration {
        let elapsed = Instant::now().duration_since(self.started_at);

        core::time::Duration::from_micros(elapsed.as_micros())
    }
}

pub struct DisplayLineBuffer<'a> {
    framebuffer: &'a mut [u8],
    stats: &'a mut RenderStats,

    line_buffer: [Rgb565Pixel; DISPLAY_WIDTH],
    transfer_buffer: [u8; DISPLAY_BATCH_BYTES],

    // Pro Zeile: kleinster und größter geänderter X-Bereich.
    dirty_start: [usize; DISPLAY_HEIGHT],
    dirty_end: [usize; DISPLAY_HEIGHT],
}

impl<'a> DisplayLineBuffer<'a> {
    pub fn new(framebuffer: &'a mut [u8], stats: &'a mut RenderStats) -> Self {
        assert_eq!(
            framebuffer.len(),
            DISPLAY_WIDTH * DISPLAY_HEIGHT * 2,
            "Invalid display framebuffer size",
        );

        Self {
            framebuffer,
            stats,
            line_buffer: [Rgb565Pixel(0); DISPLAY_WIDTH],
            transfer_buffer: [0; DISPLAY_BATCH_BYTES],
            dirty_start: [DISPLAY_WIDTH; DISPLAY_HEIGHT],
            dirty_end: [0; DISPLAY_HEIGHT],
        }
    }

    /// Überträgt die geänderten Bereiche asynchron per DMA.
    ///
    /// Zusammenhängende Zeilen mit demselben X-Bereich werden
    /// zu einem rechteckigen Display-Update zusammengefasst.
    pub async fn flush(&mut self, display: &mut DisplayDma<'_>) -> Result<(), DisplayError> {
        let mut y = 0;

        let flush_started = Instant::now();
        let copy_us_before = self.stats.copy_us;
        let transfer_us_before = self.stats.transfer_us;

        while y < DISPLAY_HEIGHT {
            let x_start = self.dirty_start[y];
            let x_end = self.dirty_end[y];

            // Diese Zeile wurde nicht verändert.
            if x_start >= x_end {
                y += 1;
                continue;
            }

            let width = x_end - x_start;
            let mut rows = 1;

            // Benachbarte Zeilen mit identischem Bereich bündeln.
            while y + rows < DISPLAY_HEIGHT
                && rows < DISPLAY_BATCH_HEIGHT
                && self.dirty_start[y + rows] == x_start
                && self.dirty_end[y + rows] == x_end
            {
                rows += 1;
            }

            let transfer_bytes = width * rows * 2;

            let copy_started = Instant::now();

            for row in 0..rows {
                let source_y = y + row;

                let source_start = (source_y * DISPLAY_WIDTH + x_start) * 2;
                let source_end = source_start + width * 2;

                let target_start = row * width * 2;
                let target_end = target_start + width * 2;

                self.transfer_buffer[target_start..target_end]
                    .copy_from_slice(&self.framebuffer[source_start..source_end]);
            }

            self.stats.copy_us += Instant::now().duration_since(copy_started).as_micros();

            let started = Instant::now();

            display
                .write_rect(
                    x_start as u16,
                    y as u16,
                    width as u16,
                    rows as u16,
                    &self.transfer_buffer[..transfer_bytes],
                )
                .await?;

            self.stats.transfer_us += Instant::now().duration_since(started).as_micros();

            self.stats.batches += 1;

            // Erst nach erfolgreicher Übertragung als sauber markieren.
            for row in y..y + rows {
                self.dirty_start[row] = DISPLAY_WIDTH;
                self.dirty_end[row] = 0;
            }

            y += rows;
        }

        let flush_elapsed = Instant::now().duration_since(flush_started).as_micros();

        let copy_us = self.stats.copy_us - copy_us_before;
        let transfer_us = self.stats.transfer_us - transfer_us_before;

        self.stats.flush_overhead_us = flush_elapsed
            .saturating_sub(copy_us)
            .saturating_sub(transfer_us);

        Ok(())
    }

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Rgb565Pixel]),
    ) {
        let process_started = Instant::now();

        if range.is_empty() {
            render_fn(&mut self.line_buffer[..0]);

            self.stats.process_line_us +=
                Instant::now().duration_since(process_started).as_micros();

            return;
        }

        assert!(
            line < DISPLAY_HEIGHT && range.end <= DISPLAY_WIDTH,
            "Slint rendered outside the display bounds",
        );

        let width = range.len();

        let render_started = Instant::now();

        render_fn(&mut self.line_buffer[..width]);

        self.stats.render_us += Instant::now().duration_since(render_started).as_micros();

        let copy_started = Instant::now();

        // Gerenderte RGB565-Pixel zeilenweise in den Framebuffer kopieren.
        let destination_start = (line * DISPLAY_WIDTH + range.start) * 2;
        let destination_end = destination_start + width * 2;

        let destination = &mut self.framebuffer[destination_start..destination_end];

        for (pixel, bytes) in self.line_buffer[..width]
            .iter()
            .zip(destination.chunks_exact_mut(2))
        {
            bytes[0] = (pixel.0 >> 8) as u8;
            bytes[1] = pixel.0 as u8;
        }

        self.stats.framebuffer_copy_us += Instant::now().duration_since(copy_started).as_micros();

        self.dirty_start[line] = self.dirty_start[line].min(range.start);
        self.dirty_end[line] = self.dirty_end[line].max(range.end);

        self.stats.lines += 1;
        self.stats.pixels += width as u32;

        self.stats.process_line_us += Instant::now().duration_since(process_started).as_micros();
    }
}

impl LineBufferProvider for DisplayLineBuffer<'_> {
    type TargetPixel = Rgb565Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        DisplayLineBuffer::process_line(self, line, range, render_fn);
    }
}

impl LineBufferProvider for &mut DisplayLineBuffer<'_> {
    type TargetPixel = Rgb565Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        (**self).process_line(line, range, render_fn);
    }
}
