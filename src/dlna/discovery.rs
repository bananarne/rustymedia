use futures::{Future, IntoFuture, Stream};
use std;
use tokio_core;

use crate::dlna;
use crate::error::ResultExt;

pub fn schedule_presence_broadcasts(
	handle: tokio_core::reactor::Handle,
	addr: std::net::SocketAddr)
{
	let socket = std::net::UdpSocket::bind("[::]:0").unwrap();
	socket.connect("239.255.255.250:1900").unwrap();
	let socket = std::rc::Rc::new(socket);
	
	let make_msg = |nt, usn: &str| format!("\
		NOTIFY * HTTP/1.1\r\n\
		HOST: 239.255.255.250:1900\r\n\
		NT: {}\r\n\
		NTS: ssdp:alive\r\n\
		LOCATION: http://{}/root.xml\r\n\
		USN: {}\r\n\
		CACHE-CONTROL: max-age=1800\r\n\
		SERVER: somesystem, DLNADOC/1.50 UPnP/1.0, rustmedia/1.0\r\n\
		\r\n",
		nt,
		addr,
		usn).into_bytes();
	
	let make_dup = |nt| make_msg(nt, format!("{}::{}", dlna::UDN, nt).as_str());
	
	let msg_root = make_dup("upnp:rootdevice");
	let msg_mediaserver = make_dup("urn:schemas-upnp-org:device:MediaServer:1");
	let msg_contentdir = make_dup("urn:schemas-upnp-org:service:ContentDirectory:1");
	let msg_connectionmanager = make_dup("urn:schemas-upnp-org:service:ConnectionManager:1");
	let msg_uuid = make_msg(dlna::UDN, dlna::UDN);
	
	let broadcast_message = move |desc, data: &[u8]| {
		socket.send(data)
			.map(|bytes_written| if bytes_written != data.len() {
				eprintln!("W: sending of {} truncated.", desc); })
			.chain_err(|| format!("Error sending {}", desc))
	};
	
	let broadcast_presence = move || -> crate::error::Result<()> {
		// eprintln!("Broadcasting presence.");
		// eprintln!("{}", String::from_utf8_lossy(&msg_uuid));
		
		// Spec recommends sending each packet 3 times. One seems fine for now.
		for _ in 0..1 {
			broadcast_message("uuid", &msg_uuid)?;
			broadcast_message("root", &msg_root)?;
			broadcast_message("mediaserver", &msg_mediaserver)?;
			broadcast_message("connectionmanager", &msg_connectionmanager)?;
			broadcast_message("contentdir", &msg_contentdir)?;
		}
		
		Ok(())
	};
	
	handle.spawn(tokio_core::reactor::Interval::new_at(
		std::time::Instant::now(),
		std::time::Duration::from_secs(10),
		&handle).unwrap()
		.for_each(move |_|
			broadcast_presence()
				.or_else(|e: crate::error::Error| {
					eprintln!("Error broadcasting presence: {:?}", e);
					Ok(())
				})
				.into_future())
		.map_err(|e| { eprintln!("Error at end of forever: {:?}", e); }));
}
