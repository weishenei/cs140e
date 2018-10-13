use core::fmt;

use volatile::prelude::*;
use volatile::{Volatile, ReadVolatile, Reserved};

use timer;
use common::IO_BASE;
use gpio::{Gpio, Function};

/// The base address for the `MU` registers.
const MU_REG_BASE: usize = IO_BASE + 0x215040;

/// The `AUXENB` register from page 9 of the BCM2837 documentation.
const AUX_ENABLES: *mut Volatile<u8> = (IO_BASE + 0x215004) as *mut Volatile<u8>;

/// Enum representing bit fields of the `AUX_MU_LSR_REG` register.
#[repr(u8)]
enum LsrStatus {
    DataReady = 1,
    TxAvailable = 1 << 5,
}

#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
    IO: Volatile<u32>, // IO read/write.
    IER: Volatile<u32>, // Interrupt enable.
    IIR: Volatile<u32>, // Interrupt status.
    LCR: Volatile<u32>, // Line data format control.
    MCR: Volatile<u32>, // Controls modem signals.
    LSR: Volatile<u32>, // Data status.
    MSR: ReadVolatile<u32>, // Modem status.
    SCRATCH: Reserved<u32>, // Scratch register, not used.
    CNTL: Volatile<u32>, // Control, provides access to additional features.
    STAT: ReadVolatile<u32>, // miniUART status.
    BAUD: Volatile<u32>, // Baud rate.
}

/// The Raspberry Pi's "mini UART".
pub struct MiniUart {
    registers: &'static mut Registers,
    timeout: Option<u32>,
}

impl MiniUart {
    /// Initializes the mini UART by enabling it as an auxiliary peripheral,
    /// setting the data size to 8 bits, setting the BAUD rate to ~115200 (baud
    /// divider of 270), setting GPIO pins 14 and 15 to alternative function 5
    /// (TXD1/RDXD1), and finally enabling the UART transmitter and receiver.
    ///
    /// By default, reads will never time out. To set a read timeout, use
    /// `set_read_timeout()`.
    pub fn new() -> MiniUart {
        let registers = unsafe {
            // Enable the mini UART as an auxiliary device.
            (*AUX_ENABLES).or_mask(1);
            &mut *(MU_REG_BASE as *mut Registers)
        };

        // FIXME: Implement remaining mini UART initialization.
        registers.LCR.write(0x3); // Enable 8-bit mode.

        // The baud register is (system_clock_rate / (8 * desired_baud) - 1)
        // For 115200, this is 270.
        registers.BAUD.write(270);

        Gpio::new(14).into_alt(Function::Alt5);
        Gpio::new(15).into_alt(Function::Alt5);

        registers.CNTL.write(0x3); // Enable RX/TX.

        MiniUart { registers, timeout: None }
    }

    /// Set the read timeout to `milliseconds` milliseconds.
    pub fn set_read_timeout(&mut self, milliseconds: u32) {
        self.timeout = Some(milliseconds);
    }

/*     ///Write char
    pub fn write_str(&mut self, str: &String) {
        let bytes: &[u8] = str.as_bytes();
        for byte in bytes {
            uart.putc(*byte);
        }
    } */

    /// Write the byte `byte`. This method blocks until there is space available
    /// in the output FIFO.
    pub fn write_byte(&mut self, byte: u8) {
         // Wait until the transmit FIFO can accept at least one byte.
        while !self.registers.LSR.has_mask(LsrStatus::TxAvailable as u32) {
            continue
        }

        self.registers.IO.write(byte as u32);
    }

    /// Returns `true` if there is at least one byte ready to be read. If this
    /// method returns `true`, a subsequent call to `read_byte` is guaranteed to
    /// return immediately. This method does not block.
    pub fn has_byte(&self) -> bool {
        self.registers.LSR.has_mask(LsrStatus::DataReady as u32)
    }

    /// Blocks until there is a byte ready to read. If a read timeout is set,
    /// this method blocks for at most that amount of time. Otherwise, this
    /// method blocks indefinitely until there is a byte to read.
    ///
    /// Returns `Ok(())` if a byte is ready to read. Returns `Err(())` if the
    /// timeout expired while waiting for a byte to be ready. If this method
    /// returns `Ok(())`, a subsequent call to `read_byte` is guaranteed to
    /// return immediately.
    pub fn wait_for_byte(&self) -> Result<(), ()> {
        let start_time: u64 = timer::current_time();

        while !self.has_byte() {
            // Check for timeout.
            if let Some(duration) = self.timeout {
                if timer::current_time() > start_time + (duration as u64) * 1000 {
                    return Err(());
                }
            }
        }

        Ok(())
    }

    /// Reads a byte. Blocks indefinitely until a byte is ready to be read.
    pub fn read_byte(&mut self) -> u8 {
        while !self.has_byte() {
            continue
        }

        (self.registers.IO.read() & 0xFF) as u8
    }
}

// FIXME: Implement `fmt::Write` for `MiniUart`. A b'\r' byte should be written
// before writing any b'\n' byte.
impl fmt::Write for MiniUart {
    /// Writes a string to the miniUart. For any \n character, a \r is
    /// automatically written preceding it.
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes: &[u8] = s.as_bytes();
        for &byte in bytes {
            // Write a \r before a \n.
            if byte == b'\n' {
                self.write_byte(b'\r');
            }

            self.write_byte(byte);
        }

        Ok(())
    }
}


#[cfg(feature = "std")]
mod uart_io {
    use std::io;
    use super::MiniUart;

    // FIXME: Implement `io::Read` and `io::Write` for `MiniUart`.
    //
    // The `io::Read::read()` implementation must respect the read timeout by
    // waiting at most that time for the _first byte_. It should not wait for
    // any additional bytes but _should_ read as many bytes as possible. If the
    // read times out, an error of kind `TimedOut` should be returned.
    //
    // The `io::Write::write()` method must write all of the requested bytes
    // before returning.
    impl io::Read for MiniUart {
        /// Waits until the timeout duration but data to arrive, and then reads
        /// any available data, up to buf.len() bytes.
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.wait_for_byte().is_err() {
                Err(io::Error::new(io::ErrorKind::TimedOut,
                                   "Timeout waiting for data"))
            } else {
                let mut bytes_read: usize = 0;
                while self.has_byte() && bytes_read < buf.len() {
                    buf[bytes_read] = self.read_byte();
                    bytes_read += 1;
                }

                Ok(bytes_read)
            }
        }
    }

    impl io::Write for MiniUart {
        /// Write the requested buffer to the miniUART, and wait for it to
        /// finish transmitting before returning.
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            for &byte in buf {
                self.write_byte(byte);
            }

            Ok(buf.len())
        }

        /// Flush the buffer (no-ops for miniUART).
        fn flush(&mut self) -> io::Result<()> {
            // Technically the miniUART may still be transmitting, but the
            // buffers are hardware buffers and will not be reset if this object
            // is destroyed, so it's safe to no-op this flush() function.
            Ok(())
        }
    }
}
