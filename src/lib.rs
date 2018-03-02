extern crate futures;
#[macro_use] extern crate log;
extern crate serialport;
extern crate tokio_io;

pub use serialport::{Result, SerialPortSettings};
pub use serialport::prelude;
use std::cmp;
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::io;
use std::thread;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct SerialPort {
    inner: Arc<Mutex<Inner>>
}

impl SerialPort {
    pub fn set_baud_rate(&mut self, baud_rate: serialport::BaudRate) -> serialport::Result<()> {
        self.inner.lock().unwrap().port.set_baud_rate(baud_rate)
    }
}

pub fn open<T: AsRef<OsStr> + ?Sized>(port: &T) -> serialport::Result<Box<SerialPort>> 
{
    let p = serialport::open(port)?;
    let inner = Arc::new(Mutex::new(Inner::new(p)));
    init_inner(inner.clone());
    Ok(Box::new(SerialPort{ inner: inner }))
}

pub fn open_with_settings<T: AsRef<OsStr> + ?Sized>(
        port: &T, 
        settings: &serialport::SerialPortSettings
        ) -> Result<Box<SerialPort>>
{
    let mut s = settings.clone();
    s.timeout = Duration::from_millis(1);
    let p = serialport::open_with_settings(port, &s)?;
    let inner = Arc::new(Mutex::new(Inner::new(p)));
    init_inner(inner.clone());
    Ok(Box::new(SerialPort{ inner: inner }))
}

fn init_inner(inner: Arc<Mutex<Inner>>) {
    thread::spawn( move || {
        loop {
            {
                // See if there are any bytes to read
                let mut buffer = [0; 256];
                let maybe_len = {
                    let mut _inner =  inner.lock().unwrap();
                    _inner.port.read(&mut buffer)
                };
                if let Ok(len) = maybe_len {
                    debug!("Read {} bytes.", len);
                    let mut _inner =  inner.lock().unwrap();
                    let (_buf, _) = buffer.split_at(len);
                    let in_buf = &mut _inner.in_buf;
                    for b in _buf.iter() {
                        in_buf.push_back(*b);
                    }
                } else {
                    thread::sleep( Duration::from_millis(1) );
                }
            }
            
            {
                // See if there are bytes to send to the serial port
                let mut _inner =  inner.lock().unwrap();
                let out_buf = _inner.out_buf.clone();
                let port = &mut _inner.port;
                if out_buf.len() > 0 {
                    port.write(out_buf.as_slice()).unwrap();
                }
            }
        }
    });
}

struct Inner {
    port: Box<serialport::SerialPort>,
    in_buf: VecDeque<u8>,
    out_buf: Vec<u8>,
}

impl Inner {
    pub fn new(port: Box<serialport::SerialPort>) -> Inner {
        Inner{
            port: port,
            in_buf: VecDeque::new(),
            out_buf: Vec::new(),
        }
    }
}

impl io::Read for SerialPort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        /*
        match self.inner.in_buf.try_recv() {
            Ok(v) => {
                if v.len() > buf.len() {
                    return Err(io::Error::new(io::ErrorKind::Other, "Buffer size too small."));
                }
                let mut i = 0;
                for b in v {
                    buf[i] = b;
                    i = i + 1;
                }
                Ok(v.len())
            }
            _ => {
                Err( io::Error::new(io::ErrorKind::WouldBlock, "No bytes in serial buffer."))
            }
        }
        */
        let mut inner = self.inner.lock().unwrap();
        let in_buf = &mut inner.in_buf;
        //let in_buf = self.inner.in_buf.lock().unwrap();
        if in_buf.len() == 0 {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "No bytes in serial buffer."));
        }

        /*
        let mut v = {
            let len = cmp::min(in_buf.len(), buf.len());
            in_buf.split_off(len)
        };
        */

        let mut i = 0;

        /*
        while let Some(b) = v.pop_front() {
            buf[i] = b;
            i = i+1;
        }
        */

        while i < buf.len() {
            if let Some(b) = in_buf.pop_front() {
                buf[i] = b;
                i = i+1;
            } else {
                break;
            }
        }
        Ok(i)
    }
}

impl io::Write for SerialPort {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        let out_buf = &mut inner.out_buf;
        if out_buf.len() > 0 {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "Serial write operation would block."));
        }
        for b in buf.iter() {
            out_buf.push(*b);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl tokio_io::AsyncRead for SerialPort { }
impl tokio_io::AsyncWrite for SerialPort {
    fn shutdown(&mut self) -> futures::Poll<(), io::Error> {
        Ok(futures::Async::Ready(()))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
