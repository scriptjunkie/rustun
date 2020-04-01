use libc::{open, ioctl, write, read};
use std::os::raw::{c_char, c_short};
use std::sync::{Arc, Mutex};
use std::ffi::CString;
use std::{io, mem, thread, env};
use std::net::{SocketAddr, UdpSocket};
const TUNSETIFF: u64 = 0x400454ca;
const SIOCGIFFLAGS: u64 = 0x8913;
const SIOCSIFFLAGS: u64 = 0x8914;
const SIOCSIFADDR: u64 = 0x8916;
const SIOCSIFNETMASK: u64 = 0x891c;
const IFF_RUNNING: c_short = 0x40;
const IFF_UP: c_short = 0x1;
const IFF_TUN: c_short = 0x0001;
const IFF_NO_PI: c_short = 0x1000;

#[repr(C, align(16))] //struct for TUNSETIFF and SIOCGIFFLAGS/SIOCSIFFLAGS ioctl's
pub struct SetIff {
    ifname: [c_char; 16],
    flags: c_short,
    slack: [u8; 128],
}
#[repr(C, align(16))] //struct for SIOCSIFADDR ioctl
pub struct SetAddr {
    ifname: [c_char; 16],
    addr: libc::sockaddr_in,
}
fn check(ret: i32) -> io::Result<()>{
    if ret != 0{
        return Err(io::Error::from_raw_os_error(ret));
    }
    Ok(())
}
fn main() -> io::Result<()>{
    let arg = env::args().nth(1).expect("Argument missing! IP:port or -s to run as server");
    let socket = if arg == "-s" {
        UdpSocket::bind("0.0.0.0:14284")? //server: bind to 14284
    } else {
        UdpSocket::bind("0.0.0.0:0")? //client: let system select port to bind to
    };
    let addr: Arc<Mutex<Option<SocketAddr>>> = if arg == "-s" { //addr will be shared by each thread
        Arc::new(Mutex::new(None)) //server doesn't know who to talk to yet
    } else {
        Arc::new(Mutex::new(Some(arg.parse().expect("Invalid IP:port")))) //client parses IP:port
    };
    let devnettun = CString::new("/dev/net/tun").unwrap();
    let fd = unsafe{open(devnettun.as_ptr(), 2)};
    let mut params = SetIff {
        ifname: [b't' as i8,b'u' as i8, b'n' as i8, b'7' as i8,0,0,0,0,0,0,0,0,0,0,0,0],//"tun7",
        flags: IFF_TUN | IFF_NO_PI,
        slack: [0; 128],
    };
    check(unsafe{ioctl(fd, TUNSETIFF, &params)})?; //Set tun flags
    let s = unsafe{libc::socket(libc::AF_INET, libc::SOCK_DGRAM, libc::IPPROTO_IP)}; //ioctl socket
    let last_octet = if arg == "-s" { 1 } else { 2 }; // server is 10.8.3.1 client is 10.8.3.2
    let mut tun_addr = SetAddr {
        ifname: params.ifname.clone(), //same interface
        addr: libc::sockaddr_in{
            sin_family: libc::AF_INET as u16,
            sin_port: 0,
            sin_addr: libc::in_addr{s_addr: u32::from_ne_bytes([10,8,3,last_octet])}, //10.8.3.X
            sin_zero: [0; 8],
        }
    };
    check(unsafe{ioctl(s, SIOCSIFADDR, &tun_addr)})?; //Set address of interface (10.8.3.X)
    check(unsafe{ioctl(s, SIOCGIFFLAGS, &params)})?; //get flags
    params.flags = params.flags | IFF_RUNNING | IFF_UP; //add UP+RUNNING to flags
    check(unsafe{ioctl(s, SIOCSIFFLAGS, &params)})?; //set flags (turns interface on)
    tun_addr.addr.sin_addr.s_addr = u32::from_ne_bytes([255,255,255,0]);
    check(unsafe{ioctl(s, SIOCSIFNETMASK, &tun_addr)})?; //set netmask (255.255.255.0)
    let addr_ref = Arc::clone(&addr); //reference to addr for recv thread to update
    let rcv_sock = socket.try_clone()?; //clone socket handle for thread to use
    thread::spawn(move || { //this thread forwards from socket to tun device
        let mut buf = [0; 65536]; //support full jumbo frames
        while let Ok((rcvd, src_addr)) = rcv_sock.recv_from(&mut buf){
            let _ = addr_ref.lock().and_then(|mut a|Ok(a.replace(src_addr))); //save addr
            let origptr: *const u8 = &buf[0]; //we'll need transmute to convert *const u8 to void*
            let res = unsafe{write(fd, mem::transmute(origptr), rcvd)}; //send it along
            if res != rcvd as isize{
                eprintln!("Error? {} vs expected {}", res, rcvd);
                std::process::exit(1) //exit if write fails
            }
        }
    });
    let mut buf = [0; 65536]; //support full jumbo frames
    let buf_u8_ptr: *mut u8 = &mut buf[0]; //we'll need transmute to convert *mut u8 to void*
    loop {
        let read_res = unsafe{read(fd, mem::transmute(buf_u8_ptr), 65536)};
        if read_res <= 0 {
            eprintln!("Error? read {}", read_res);
            break;
        }
        if let Ok(addr_guard) = addr.lock() {
            if let Some(addr_real) = *addr_guard { //server sends to last seen addr
                socket.send_to(&buf[0..read_res as usize], addr_real)?;
            }
        }
    }
    Ok(())
}